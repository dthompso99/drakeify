// Tool Management Module

/**
 * Load installed tools
 */
function loadInstalledTools() {
    if (!authToken) {
        document.getElementById('installed-tools-list').innerHTML =
            '<div class="error">Please authenticate first</div>';
        return;
    }

    fetch('/api/tools', {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Failed to load tools');
        return r.json();
    })
    .then(tools => {
        const list = document.getElementById('installed-tools-list');
        if (tools.length === 0) {
            list.innerHTML = '<div class="empty-state">No tools installed. Switch to the "Available" tab to install tools.</div>';
        } else {
            list.innerHTML = '<table><thead><tr><th>Name</th><th>Version</th><th>Description</th><th>Author</th><th>Actions</th></tr></thead><tbody>' +
                tools.map(t => `<tr>
                    <td><strong>${t.name}</strong></td>
                    <td>${t.version}</td>
                    <td>${t.description}</td>
                    <td>${t.author || 'Unknown'}</td>
                    <td>
                        <button onclick="showToolConfigModal('${t.name}')" class="btn btn-sm" style="background: #3b82f6;">Configure</button>
                        <button onclick="uninstallTool('${t.name}')" class="btn btn-sm" style="background: #ef4444;">Uninstall</button>
                    </td>
                </tr>`).join('') +
                '</tbody></table>' +
                '<div style="margin-top: 1rem; padding: 1rem; background: #334155; border-radius: 8px; color: #94a3b8;">' +
                '<strong>\u26a0\ufe0f Note:</strong> After installing or uninstalling tools, you must restart the drakeify proxy for changes to take effect.' +
                '</div>';
        }
    })
    .catch(err => {
        console.error('Error loading installed tools:', err);
        document.getElementById('installed-tools-list').innerHTML =
            '<div class="error">Failed to load installed tools</div>';
    });
}

/**
 * Load available tools from registry
 */
async function loadAvailableTools() {
    if (!authToken) {
        document.getElementById('available-tools-list').innerHTML =
            '<div class="error">Please authenticate first</div>';
        return;
    }

    try {
        const response = await fetch('/api/tools/available', {
            headers: { 'Authorization': 'Bearer ' + authToken }
        });

        if (!response.ok) throw new Error('Failed to load available tools');

        const packages = await response.json();
        const list = document.getElementById('available-tools-list');

        if (packages.length === 0) {
            list.innerHTML = '<div class="empty-state">No tools available in the registry.</div>';
            return;
        }

        // Fetch latest version for each tool
        const toolsWithVersions = await Promise.all(
            packages.map(async (name) => {
                try {
                    const tagsResponse = await fetch(`/api/tools/${name}/tags`, {
                        headers: { 'Authorization': 'Bearer ' + authToken }
                    });

                    if (!tagsResponse.ok) {
                        return { name, version: 'unknown' };
                    }

                    const tagsData = await tagsResponse.json();
                    const tags = tagsData.tags || [];
                    const latestTag = tags.length > 0 ? tags[tags.length - 1] : 'unknown';

                    return { name, version: latestTag };
                } catch (err) {
                    console.error(`Error fetching tags for ${name}:`, err);
                    return { name, version: 'unknown' };
                }
            })
        );

        // Render table with versions
        list.innerHTML = '<table><thead><tr><th>Name</th><th>Latest Version</th><th>Actions</th></tr></thead><tbody>' +
            toolsWithVersions.map(tool => `<tr>
                <td><strong>${tool.name}</strong></td>
                <td><span style="color: #6b7280; font-family: monospace;">${tool.version}</span></td>
                <td>
                    <button onclick="showInstallToolForm('${tool.name}')" class="btn btn-sm" style="background: #10b981;">Install</button>
                </td>
            </tr>`).join('') +
            '</tbody></table>';

    } catch (err) {
        console.error('Error loading available tools:', err);
        document.getElementById('available-tools-list').innerHTML =
            '<div class="error">Failed to load available tools. Make sure the registry is accessible.</div>';
    }
}

/**
 * Load unpublished tools
 */
function loadUnpublishedTools() {
    if (!authToken) {
        document.getElementById('publish-tools-list').innerHTML =
            '<div class="error">Please authenticate first</div>';
        return;
    }

    fetch('/api/tools/unpublished', {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Failed to load unpublished tools');
        return r.json();
    })
    .then(tools => {
        const list = document.getElementById('publish-tools-list');
        if (tools.length === 0) {
            list.innerHTML = '<div class="empty-state">No unpublished tools found in /data/tools/.<br><br>Tools must have a metadata.json file to be publishable.</div>';
        } else {
            list.innerHTML = '<table><thead><tr><th>Name</th><th>Version</th><th>Description</th><th>Path</th><th>Actions</th></tr></thead><tbody>' +
                tools.map(t => `<tr>
                    <td><strong>${t.name}</strong></td>
                    <td>${t.version}</td>
                    <td>${t.description}</td>
                    <td><code style="font-size: 0.875rem;">${t.path}</code></td>
                    <td>
                        <button onclick="publishTool('${t.path}')" class="btn btn-sm" style="background: #8b5cf6;">Publish</button>
                    </td>
                </tr>`).join('') +
                '</tbody></table>';
        }
    })
    .catch(err => {
        console.error('Error loading unpublished tools:', err);
        document.getElementById('publish-tools-list').innerHTML =
            '<div class="error">Failed to load unpublished tools.</div>';
    });
}

/**
 * Show install tool form with version selection
 * @param {string} name - Tool name
 */
async function showInstallToolForm(name) {
    // Fetch available tags from the registry
    let defaultVersion = '1.0.0';
    let availableVersions = [];

    try {
        const response = await fetch(`/api/tools/${name}/tags`, {
            headers: { 'Authorization': 'Bearer ' + authToken }
        });

        if (response.ok) {
            const data = await response.json();
            if (data.tags && data.tags.length > 0) {
                availableVersions = data.tags;
                defaultVersion = data.tags[data.tags.length - 1]; // Latest version
            }
        }
    } catch (err) {
        console.error('Error fetching tool tags:', err);
    }

    const version = prompt(`Install tool "${name}".\n\nEnter version (available: ${availableVersions.join(', ') || defaultVersion}):`, defaultVersion);
    if (version) {
        installTool(name, version);
    }
}

/**
 * Install a tool
 * @param {string} name - Tool name
 * @param {string} version - Tool version
 */
function installTool(name, version) {
    if (!confirm(`Install tool "${name}" version "${version}"?\n\nNote: You will need to restart the drakeify proxy after installation.`)) {
        return;
    }

    fetch('/api/tools/install', {
        method: 'POST',
        headers: {
            'Authorization': 'Bearer ' + authToken,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ name, version })
    })
    .then(r => {
        if (!r.ok) throw new Error('Installation failed');
        return r.json();
    })
    .then(() => {
        alert(`Tool "${name}" installed successfully!\n\nPlease restart the drakeify proxy to enable the tool.`);
        loadInstalledTools();
        loadAvailableTools();
    })
    .catch(err => {
        console.error('Error installing tool:', err);
        alert('Failed to install tool. Check the console for details.');
    });
}

/**
 * Uninstall a tool
 * @param {string} name - Tool name
 */
function uninstallTool(name) {
    if (!confirm(`Uninstall tool "${name}"?\n\nNote: You will need to restart the drakeify proxy after uninstallation.`)) {
        return;
    }

    fetch(`/api/tools/${name}`, {
        method: 'DELETE',
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Uninstall failed');
        alert(`Tool "${name}" uninstalled successfully!\n\nPlease restart the drakeify proxy to apply changes.`);
        loadInstalledTools();
    })
    .catch(err => {
        console.error('Error uninstalling tool:', err);
        alert('Failed to uninstall tool');
    });
}

/**
 * Publish a tool to the registry
 * @param {string} path - Tool path
 */
function publishTool(path) {
    if (!confirm(`Publish tool from "${path}" to the registry?\n\nThis will make it available for installation.`)) {
        return;
    }

    fetch('/api/tools/publish', {
        method: 'POST',
        headers: {
            'Authorization': 'Bearer ' + authToken,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({ path })
    })
    .then(r => {
        if (!r.ok) throw new Error('Publish failed');
        return r.json();
    })
    .then(() => {
        alert(`Tool published successfully!`);
        loadUnpublishedTools();
        loadAvailableTools();
    })
    .catch(err => {
        console.error('Error publishing tool:', err);
        alert('Failed to publish tool. Check the console for details.');
    });
}

/**
 * Show tool configuration modal
 * @param {string} toolName - Tool name
 */
async function showToolConfigModal(toolName) {
    try {
        // Fetch metadata and current config in parallel
        const [metadataRes, configRes] = await Promise.all([
            fetch(`/api/tools/${toolName}/metadata`, {
                headers: { 'Authorization': 'Bearer ' + authToken }
            }),
            fetch(`/api/tools/${toolName}/config`, {
                headers: { 'Authorization': 'Bearer ' + authToken }
            })
        ]);

        const metadata = metadataRes.ok ? await metadataRes.json() : {};
        const currentConfig = configRes.ok ? await configRes.json() : {};

        // Build configuration form
        const configSchema = metadata.config_schema || {};
        const secretsSchema = metadata.secrets_schema || {};

        let formHtml = '';

        // Configuration fields
        if (Object.keys(configSchema).length > 0) {
            formHtml += '<h3 style="color: #f1f5f9; margin-top: 0;">Configuration</h3>';
            for (const [key, schema] of Object.entries(configSchema)) {
                const value = currentConfig[key] !== undefined ? currentConfig[key] : schema.default;
                const required = schema.required ? ' *' : '';

                formHtml += `
                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; color: #cbd5e1; margin-bottom: 0.25rem; font-size: 14px;">
                            ${key}${required}
                        </label>
                        <div style="color: #94a3b8; font-size: 12px; margin-bottom: 0.5rem;">${schema.description || ''}</div>
                        ${generateFormField(key, schema, value)}
                    </div>
                `;
            }
        }

        // Secrets fields
        if (Object.keys(secretsSchema).length > 0) {
            formHtml += '<h3 style="color: #f1f5f9; margin-top: 1.5rem; border-top: 1px solid #334155; padding-top: 1rem;">Secrets</h3>';
            formHtml += '<p style="color: #94a3b8; font-size: 12px; margin-bottom: 1rem;">Secrets are stored securely and never displayed after saving.</p>';
            for (const [key, schema] of Object.entries(secretsSchema)) {
                const required = schema.required ? ' *' : '';
                formHtml += `
                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; color: #cbd5e1; margin-bottom: 0.25rem; font-size: 14px;">
                            ${key}${required}
                        </label>
                        <div style="color: #94a3b8; font-size: 12px; margin-bottom: 0.5rem;">${schema.description || ''}</div>
                        <input type="password" data-secret-key="${key}" placeholder="Enter secret value (leave empty to keep current)" style="width: 100%; background: #0f172a; color: #e2e8f0; border: 1px solid #334155; border-radius: 6px; padding: 0.5rem; font-size: 14px;" />
                    </div>
                `;
            }
        }

        if (Object.keys(configSchema).length === 0 && Object.keys(secretsSchema).length === 0) {
            formHtml += '<p style="color: #94a3b8;">This tool has no configurable options.</p>';
        }

        // Show modal
        const modal = document.createElement('div');
        modal.style.cssText = 'position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 1000;';
        modal.innerHTML = `
            <div style="background: #1e293b; padding: 2rem; border-radius: 12px; max-width: 700px; width: 90%; max-height: 80vh; overflow: auto;">
                <h2 style="margin-top: 0; color: #f1f5f9;">Configure: ${toolName}</h2>
                <form id="tool-config-form">
                    ${formHtml}
                </form>
                <div style="margin-top: 1.5rem; display: flex; gap: 0.5rem; justify-content: flex-end; border-top: 1px solid #334155; padding-top: 1rem;">
                    <button type="button" onclick="this.closest('div').parentElement.remove()" class="btn" style="background: #64748b;">Cancel</button>
                    <button type="button" onclick="saveToolConfig('${toolName}')" class="btn" style="background: #10b981;">Save</button>
                </div>
            </div>
        `;
        document.body.appendChild(modal);

        // Close on background click
        modal.addEventListener('click', (e) => {
            if (e.target === modal) modal.remove();
        });
    } catch (err) {
        console.error('Error loading tool config:', err);
        alert('Failed to load tool configuration');
    }
}

/**
 * Save tool configuration
 * @param {string} toolName - Tool name
 */
async function saveToolConfig(toolName) {
    try {
        // Collect configuration values
        const config = {};
        document.querySelectorAll('[data-config-key]').forEach(input => {
            const key = input.getAttribute('data-config-key');
            let value = input.value;

            // Type conversion
            if (input.type === 'number') {
                value = parseFloat(value);
            } else if (input.type === 'checkbox') {
                value = input.checked;
            } else if (input.getAttribute('data-type') === 'array') {
                value = value.split(',').map(v => v.trim()).filter(v => v);
            } else if (input.getAttribute('data-type') === 'object') {
                try {
                    value = JSON.parse(value);
                } catch (e) {
                    alert(`Invalid JSON for field "${key}"`);
                    return;
                }
            }

            // Only include non-empty values
            if (value !== '' && value !== null && value !== undefined) {
                config[key] = value;
            }
        });

        // Save config to server
        const configResponse = await fetch(`/api/tools/${toolName}/config`, {
            method: 'PUT',
            headers: {
                'Authorization': 'Bearer ' + authToken,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(config)
        });

        if (!configResponse.ok) {
            throw new Error('Failed to save configuration');
        }

        // Collect and save secrets
        const secretInputs = document.querySelectorAll('[data-secret-key]');
        const secretPromises = [];

        secretInputs.forEach(input => {
            const key = input.getAttribute('data-secret-key');
            const value = input.value;

            if (value && value.trim()) {
                secretPromises.push(
                    fetch(`/api/secrets/${key}`, {
                        method: 'PUT',
                        headers: {
                            'Authorization': 'Bearer ' + authToken,
                            'Content-Type': 'application/json'
                        },
                        body: JSON.stringify({ value: value.trim() })
                    })
                );
            }
        });

        if (secretPromises.length > 0) {
            await Promise.all(secretPromises);
        }

        alert(`\u2713 Configuration and ${secretPromises.length} secret(s) for "${toolName}" saved successfully!`);

        // Close the modal
        const modal = document.querySelector('div[style*="position: fixed"]');
        if (modal) modal.remove();

        loadInstalledTools();
    } catch (err) {
        console.error('Error saving tool config:', err);
        alert(`Failed to save configuration: ${err.message}`);
    }
}

