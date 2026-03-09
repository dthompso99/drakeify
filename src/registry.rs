use anyhow::{Context, Result, anyhow};
use oci_distribution::{Client, Reference, secrets::RegistryAuth};
use oci_distribution::client::{ImageLayer, Config};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;
use flate2::Compression;
use flate2::write::GzEncoder;
use flate2::read::GzDecoder;
use tar::Builder;
use sha2::{Sha256, Digest};
use tracing::{info, debug, warn};

/// Metadata for a plugin or tool package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    #[serde(rename = "type")]
    pub package_type: PackageType,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drakeify_version: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PackageType {
    Plugin,
    Tool,
}

/// Registry client for publishing and installing plugins/tools
pub struct RegistryClient {
    client: Client,
    registry_url: String,
    auth: RegistryAuth,
}

impl RegistryClient {
    /// Create a new registry client
    pub fn new(
        registry_url: String,
        username: Option<String>,
        password: Option<String>,
        insecure: bool,
    ) -> Result<Self> {
        let protocol = if insecure {
            oci_distribution::client::ClientProtocol::Http
        } else {
            oci_distribution::client::ClientProtocol::Https
        };

        let config = oci_distribution::client::ClientConfig {
            protocol,
            ..Default::default()
        };

        let client = Client::new(config);

        let auth = match (username, password) {
            (Some(u), Some(p)) => RegistryAuth::Basic(u, p),
            _ => RegistryAuth::Anonymous,
        };

        Ok(Self {
            client,
            registry_url,
            auth,
        })
    }

    /// Publish a plugin or tool to the registry
    pub async fn publish(
        &mut self,
        package_path: &Path,
        metadata: PackageMetadata,
    ) -> Result<String> {
        info!("Publishing {} '{}' version {}", 
            if metadata.package_type == PackageType::Plugin { "plugin" } else { "tool" },
            metadata.name,
            metadata.version
        );

        // Create a tarball of the package
        let tarball = self.create_package_tarball(package_path, &metadata)?;

        // Calculate SHA256 of the tarball
        let mut hasher = Sha256::new();
        hasher.update(&tarball);
        let digest = format!("sha256:{:x}", hasher.finalize());

        // Create OCI image reference
        let reference = self.create_reference(&metadata)?;

        debug!("Pushing to reference: {}", reference);

        // Create an ImageLayer from the tarball
        let layer = ImageLayer::new(
            tarball,
            "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
            None,
        );

        // Create a minimal config
        let config = Config {
            data: serde_json::to_vec(&metadata)?,
            media_type: "application/vnd.oci.image.config.v1+json".to_string(),
            annotations: None,
        };

        // Push the package as an OCI artifact
        self.client
            .push(&reference, &[layer], config, &self.auth, None)
            .await
            .context("Failed to push package to registry")?;

        info!("✓ Successfully published {}/{}:{}", 
            if metadata.package_type == PackageType::Plugin { "plugin" } else { "tool" },
            metadata.name,
            metadata.version
        );

        Ok(digest)
    }

    /// Create a tarball from the package directory
    fn create_package_tarball(
        &self,
        package_path: &Path,
        metadata: &PackageMetadata,
    ) -> Result<Vec<u8>> {
        let mut tar_data = Vec::new();
        {
            let encoder = GzEncoder::new(&mut tar_data, Compression::default());
            let mut tar = Builder::new(encoder);

            // Add the main file (plugin.js or tool.js)
            let main_file = if metadata.package_type == PackageType::Plugin {
                "plugin.js"
            } else {
                "tool.js"
            };

            let main_path = package_path.join(main_file);
            if !main_path.exists() {
                return Err(anyhow!("Main file {} not found in package", main_file));
            }

            tar.append_path_with_name(&main_path, main_file)?;

            // Add metadata.json
            let metadata_json = serde_json::to_string_pretty(metadata)?;
            let metadata_bytes = metadata_json.as_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_size(metadata_bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "metadata.json", metadata_bytes)?;

            // Add README.md if it exists
            let readme_path = package_path.join("README.md");
            if readme_path.exists() {
                tar.append_path_with_name(&readme_path, "README.md")?;
            }

            tar.finish()?;
        }

        Ok(tar_data)
    }

    /// Create an OCI reference for the package
    fn create_reference(&self, metadata: &PackageMetadata) -> Result<Reference> {
        let prefix = if metadata.package_type == PackageType::Plugin {
            "plugins"
        } else {
            "tools"
        };

        // Strip protocol from registry URL to get just the hostname
        let registry = self.registry_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches("/")
            .to_string();

        if registry.is_empty() {
            return Err(anyhow!("Registry URL is empty. Please set registry_url in drakeify.toml or DRAKEIFY_REGISTRY_URL environment variable"));
        }

        let repository = format!("{}/{}", prefix, metadata.name);

        debug!("Creating reference: registry={}, repository={}, tag={}", registry, repository, metadata.version);

        let reference = Reference::with_tag(registry, repository, metadata.version.clone());

        debug!("Created reference: {}", reference);

        Ok(reference)
    }

    /// Install a plugin or tool from the registry
    pub async fn install(
        &mut self,
        package_type: PackageType,
        name: &str,
        version: &str,
        install_dir: &Path,
    ) -> Result<PackageMetadata> {
        info!("Installing {} '{}' version {}",
            if package_type == PackageType::Plugin { "plugin" } else { "tool" },
            name,
            version
        );

        // Create reference
        let metadata = PackageMetadata {
            package_type: package_type.clone(),
            name: name.to_string(),
            version: version.to_string(),
            description: String::new(),
            author: None,
            license: None,
            homepage: None,
            dependencies: Default::default(),
            drakeify_version: None,
            tags: vec![],
            created: String::new(),
            default_config: None,
            config_schema: None,
            secrets_schema: None,
        };

        let reference = self.create_reference(&metadata)?;

        debug!("Pulling from reference: {}", reference);

        // Pull the package - this returns ImageData with layers
        let image_data = self.client
            .pull(&reference, &self.auth, vec!["application/vnd.oci.image.layer.v1.tar+gzip"])
            .await
            .context("Failed to pull package from registry")?;

        if image_data.layers.is_empty() {
            return Err(anyhow!("Package has no layers"));
        }

        // Extract the tarball from the first layer
        let tarball_data = &image_data.layers[0].data;
        let decoder = GzDecoder::new(&tarball_data[..]);
        let mut archive = tar::Archive::new(decoder);

        // Extract to install directory
        let package_dir = install_dir.join(name);
        fs::create_dir_all(&package_dir)?;

        archive.unpack(&package_dir)?;

        // Read and return metadata
        let metadata_path = package_dir.join("metadata.json");
        let metadata_content = fs::read_to_string(&metadata_path)
            .context("Failed to read metadata.json from package")?;
        let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;

        info!("✓ Successfully installed {}/{} to {}",
            if package_type == PackageType::Plugin { "plugin" } else { "tool" },
            name,
            package_dir.display()
        );

        Ok(metadata)
    }

    /// List available packages in the registry
    pub async fn list(
        &mut self,
        _package_type: PackageType,
    ) -> Result<Vec<String>> {
        // Note: OCI registries don't have a standard list API
        // This would require registry-specific implementation
        // For now, return a placeholder
        warn!("List operation not yet implemented for OCI registries");
        Ok(vec![])
    }

    /// Discover available packages in the registry using the catalog endpoint
    pub async fn discover(
        &self,
        package_type: Option<PackageType>,
    ) -> Result<Vec<String>> {
        // Use the OCI catalog endpoint to list repositories
        let catalog_url = format!("{}/v2/_catalog", self.registry_url);

        debug!("Fetching catalog from: {}", catalog_url);

        let response = reqwest::get(&catalog_url)
            .await
            .with_context(|| format!("Failed to fetch catalog from {}", catalog_url))?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch catalog: HTTP {}", response.status()));
        }

        let catalog: serde_json::Value = response.json().await?;
        let repositories = catalog["repositories"]
            .as_array()
            .ok_or_else(|| anyhow!("Invalid catalog response: missing 'repositories' array"))?;

        let mut packages = Vec::new();

        for repo in repositories {
            let repo_name = repo.as_str()
                .ok_or_else(|| anyhow!("Invalid repository name in catalog"))?;

            // Filter by package type if specified
            if let Some(ref pkg_type) = package_type {
                let prefix = match pkg_type {
                    PackageType::Plugin => "plugins/",
                    PackageType::Tool => "tools/",
                };

                if repo_name.starts_with(prefix) {
                    // Remove the prefix to get just the package name
                    packages.push(repo_name.strip_prefix(prefix).unwrap().to_string());
                }
            } else {
                // Return all packages with their type prefix
                packages.push(repo_name.to_string());
            }
        }

        Ok(packages)
    }
}


