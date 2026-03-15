// Plugin Management Module

/**
 * Show a specific plugin tab
 * @param {string} tabName - Tab name ('installed' or 'available')
 */
function showPluginTab(tabName) {
    // Update tab buttons
    document.querySelectorAll('.plugin-tab').forEach(tab => {
        tab.style.borderBottom = 'none';
        tab.style.color = '#94a3b8';
    });
    document.getElementById(`tab-${tabName}`).style.borderBottom = '2px solid #3b82f6';
    document.getElementById(`tab-${tabName}`).style.color = '#3b82f6';

    // Show/hide tab content
    document.querySelectorAll('.plugin-tab-content').forEach(content => {
        content.style.display = 'none';
    });
    document.getElementById(`${tabName}-plugins-tab`).style.display = 'block';
}

/**
 * Load installed plugins
 */
function loadInstalledPlugins() {
    if (!authToken) {
        document.getElementById('installed-plugins-list').innerHTML =
            '<div class="error">Please authenticate first</div>';
        return;
    }

    fetch('/api/plugins', {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Failed to load plugins');
        return r.json();
    })
    .then(plugins => {
        const list = document.getElementById('installed-plugins-list');
        if (plugins.length === 0) {
            list.innerHTML = '<div class="empty-state">No plugins installed. Switch to the "Available" tab to install plugins.</div>';
        } else {
            list.innerHTML = '<table><thead><tr><th>Name</th><th>Version</th><th>Description</th><th>Author</th><th>Actions</th></tr></thead><tbody>' +
                plugins.map(p => `<tr>
                    <td><strong>${p.name}</strong></td>
                    <td>${p.version}</td>
                    <td>${p.description}</td>
                    <td>${p.author || 'Unknown'}</td>
                    <td>
                        <button onclick="showPluginConfigModal('${p.name}')" class="btn btn-sm" style="background: #3b82f6; margin-right: 0.5rem;">Configure</button>
                        <button onclick="uninstallPlugin('${p.name}')" class="btn btn-sm" style="background: #ef4444;">Uninstall</button>
                    </td>
                </tr>`).join('') +
                '</tbody></table>' +
                '<div style="margin-top: 1rem; padding: 1rem; background: #334155; border-radius: 8px; color: #94a3b8;">' +
                '<strong>⚠️ Note:</strong> After installing or uninstalling plugins, you must restart the drakeify proxy for changes to take effect.' +
                '</div>';
        }
    })
    .catch(err => {
        console.error('Error loading installed plugins:', err);
        document.getElementById('installed-plugins-list').innerHTML =
            '<div class="error">Failed to load installed plugins</div>';
    });
}

/**
 * Load available plugins from registry
 */
async function loadAvailablePlugins() {
    if (!authToken) {
        document.getElementById('available-plugins-list').innerHTML =
            '<div class="error">Please authenticate first</div>';
        return;
    }

    try {
        const response = await fetch('/api/plugins/available', {
            headers: { 'Authorization': 'Bearer ' + authToken }
        });

        if (!response.ok) throw new Error('Failed to load available plugins');

        const packages = await response.json();
        const list = document.getElementById('available-plugins-list');

        if (packages.length === 0) {
            list.innerHTML = '<div class="empty-state">No plugins available in the registry.</div>';
            return;
        }

        // Fetch latest version for each plugin
        const pluginsWithVersions = await Promise.all(
            packages.map(async (name) => {
                try {
                    const tagsResponse = await fetch(`/api/plugins/${name}/tags`, {
                        headers: { 'Authorization': 'Bearer ' + authToken }
                    });

                    if (!tagsResponse.ok) {
                        return { name, version: 'unknown' };
                    }

                    const tagsData = await tagsResponse.json();
                    const tags = tagsData.tags || [];

                    if (tags.length === 0) {
                        return { name, version: 'unknown' };
                    }

                    // Sort tags semantically and get the latest
                    const sortedTags = tags.sort((a, b) => {
                        const aParts = a.split('.').map(Number);
                        const bParts = b.split('.').map(Number);
                        for (let i = 0; i < Math.max(aParts.length, bParts.length); i++) {
                            const aVal = aParts[i] || 0;
                            const bVal = bParts[i] || 0;
                            if (aVal !== bVal) return bVal - aVal;
                        }
                        return 0;
                    });

                    return { name, version: sortedTags[0] || 'unknown' };
                } catch (err) {
                    console.error(`Error fetching tags for ${name}:`, err);
                    return { name, version: 'unknown' };
                }
            })
        );

        // Render table with versions
        list.innerHTML = '<table><thead><tr><th>Name</th><th>Latest Version</th><th>Actions</th></tr></thead><tbody>' +
            pluginsWithVersions.map(plugin => `<tr>
                <td><strong>${plugin.name}</strong></td>
                <td><span style="color: #6b7280; font-family: monospace;">${plugin.version}</span></td>
                <td>
                    <button onclick="showInstallPluginForm('${plugin.name}')" class="btn btn-sm" style="background: #10b981;">Install</button>
                </td>
            </tr>`).join('') +
            '</tbody></table>';

    } catch (err) {
        console.error('Error loading available plugins:', err);
        document.getElementById('available-plugins-list').innerHTML =
            '<div class="error">Failed to load available plugins. Make sure the registry is accessible.</div>';
    }
}

/**
 * Show install plugin form with version selection
 * @param {string} name - Plugin name
 */
async function showInstallPluginForm(name) {
    // Fetch available tags from the registry
    let defaultVersion = '1.0.0';
    let availableVersions = [];

    try {
        const response = await fetch(`/api/plugins/${name}/tags`, {
            headers: { 'Authorization': 'Bearer ' + authToken }
        });

        if (response.ok) {
            const data = await response.json();
            availableVersions = data.tags || [];

            // Sort versions semantically and pick the latest
            if (availableVersions.length > 0) {
                defaultVersion = availableVersions.sort((a, b) => {
                    const aParts = a.split('.').map(Number);
                    const bParts = b.split('.').map(Number);
                    for (let i = 0; i < Math.max(aParts.length, bParts.length); i++) {
                        const aVal = aParts[i] || 0;
                        const bVal = bParts[i] || 0;
                        if (aVal !== bVal) return bVal - aVal; // Descending order
                    }
                    return 0;
                })[0];
            }
        }
    } catch (err) {
        console.warn('Failed to fetch tags for plugin:', err);
    }

    const versionList = availableVersions.length > 0
        ? `\n\nAvailable versions: ${availableVersions.join(', ')}`
        : '';

    const version = prompt(`Enter version to install for "${name}":${versionList}`, defaultVersion);
    if (!version) return;

    installPlugin(name, version);
}

/**
 * Install a plugin
 * @param {string} name - Plugin name
 * @param {string} version - Plugin version
 */
function installPlugin(name, version) {
    if (!confirm(`Install plugin "${name}" version "${version}"?\n\nNote: You will need to restart the drakeify proxy after installation.`)) {
        return;
    }

    fetch('/api/plugins/install', {
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
        alert(`Plugin "${name}" installed successfully!\n\nPlease restart the drakeify proxy to enable the plugin.`);
        loadInstalledPlugins();
        loadAvailablePlugins();
    })
    .catch(err => {
        console.error('Error installing plugin:', err);
        alert('Failed to install plugin. Check the console for details.');
    });
}

/**
 * Uninstall a plugin
 * @param {string} name - Plugin name
 */
function uninstallPlugin(name) {
    if (!confirm(`Uninstall plugin "${name}"?\n\nNote: You will need to restart the drakeify proxy after uninstallation.`)) {
        return;
    }

    fetch(`/api/plugins/${name}`, {
        method: 'DELETE',
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Uninstall failed');
        alert(`Plugin "${name}" uninstalled successfully!\n\nPlease restart the drakeify proxy to apply changes.`);
        loadInstalledPlugins();
    })
    .catch(err => {
        console.error('Error uninstalling plugin:', err);
        alert('Failed to uninstall plugin');
    });
}

/**
 * Show plugin configuration modal
 * @param {string} pluginName - Plugin name
 */
async function showPluginConfigModal(pluginName) {
    try {
        // Fetch metadata and current config in parallel
        const [metadataRes, configRes] = await Promise.all([
            fetch(`/api/plugins/${pluginName}/metadata`, {
                headers: { 'Authorization': 'Bearer ' + authToken }
            }),
            fetch(`/api/plugins/${pluginName}/config`, {
                headers: { 'Authorization': 'Bearer ' + authToken }
            })
        ]);

        const metadata = metadataRes.ok ? await metadataRes.json() : {};
        const config = configRes.ok ? await configRes.json() : {};

        const configSchema = metadata.config_schema || {};
        const secretsSchema = metadata.secrets_schema || {};

        // Build form HTML
        let formHtml = '';

        // Configuration fields
        if (Object.keys(configSchema).length > 0) {
            formHtml += '<h3 style="color: #f1f5f9; margin-top: 0;">Configuration</h3>';
            for (const [key, schema] of Object.entries(configSchema)) {
                const value = config[key] !== undefined ? config[key] : schema.default;
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
        } else {
            formHtml += '<p style="color: #94a3b8;">No configuration fields defined for this plugin.</p>';
        }

        // Secrets fields
        if (Object.keys(secretsSchema).length > 0) {
            formHtml += '<h3 style="color: #f1f5f9; margin-top: 1.5rem; border-top: 1px solid #334155; padding-top: 1rem;">Secrets</h3>';
            formHtml += '<p style="color: #94a3b8; font-size: 12px; margin-bottom: 1rem;">⚠️ Secrets are stored separately and securely. They are not shown here.</p>';
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

        // Show modal
        const modal = document.createElement('div');
        modal.style.cssText = 'position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.8); display: flex; align-items: center; justify-content: center; z-index: 1000;';
        modal.innerHTML = `
            <div style="background: #1e293b; padding: 2rem; border-radius: 12px; max-width: 700px; width: 90%; max-height: 80vh; overflow: auto;">
                <h2 style="margin-top: 0; color: #f1f5f9;">Configure: ${pluginName}</h2>
                <form id="plugin-config-form">
                    ${formHtml}
                </form>
                <div style="margin-top: 1.5rem; display: flex; gap: 0.5rem; justify-content: flex-end; border-top: 1px solid #334155; padding-top: 1rem;">
                    <button type="button" onclick="this.closest('div').parentElement.remove()" class="btn" style="background: #64748b;">Cancel</button>
                    <button type="button" onclick="savePluginConfig('${pluginName}')" class="btn" style="background: #10b981;">Save</button>
                </div>
            </div>
        `;
        document.body.appendChild(modal);

        // Close on background click
        modal.addEventListener('click', (e) => {
            if (e.target === modal) modal.remove();
        });

    } catch (err) {
        console.error('Error loading plugin config:', err);
        alert('Failed to load plugin configuration');
    }
}

/**
 * Generate form field based on schema
 * @param {string} key - Field key
 * @param {Object} schema - Field schema
 * @param {*} value - Current value
 * @returns {string} HTML for form field
 */
function generateFormField(key, schema, value) {
    const inputStyle = 'width: 100%; background: #0f172a; color: #e2e8f0; border: 1px solid #334155; border-radius: 6px; padding: 0.5rem; font-size: 14px;';

    switch (schema.type) {
        case 'boolean':
            return `<input type="checkbox" data-config-key="${key}" ${value ? 'checked' : ''} style="width: auto; height: 20px; cursor: pointer;" />`;
        case 'number':
            return `<input type="number" data-config-key="${key}" value="${value !== undefined ? value : ''}" style="${inputStyle}" />`;
        case 'array':
        case 'object':
            return `<textarea data-config-key="${key}" rows="5" style="${inputStyle}">${JSON.stringify(value, null, 2)}</textarea>`;
        case 'string':
        default:
            return `<input type="text" data-config-key="${key}" value="${value !== undefined ? value : ''}" style="${inputStyle}" />`;
    }
}

/**
 * Save plugin configuration
 * @param {string} pluginName - Plugin name
 */
async function savePluginConfig(pluginName) {
    try {
        const form = document.getElementById('plugin-config-form');
        const config = {};

        // Collect config values from form
        form.querySelectorAll('[data-config-key]').forEach(input => {
            const key = input.getAttribute('data-config-key');
            let value;

            if (input.type === 'checkbox') {
                value = input.checked;
            } else if (input.type === 'number') {
                value = input.value ? parseFloat(input.value) : undefined;
            } else if (input.tagName === 'TEXTAREA') {
                // Parse JSON for arrays/objects
                try {
                    value = JSON.parse(input.value);
                } catch (e) {
                    throw new Error(`Invalid JSON in field "${key}"`);
                }
            } else {
                value = input.value;
            }

            if (value !== undefined && value !== '') {
                config[key] = value;
            }
        });

        // Save config to server
        const configResponse = await fetch(`/api/plugins/${pluginName}/config`, {
            method: 'PUT',
            headers: {
                'Authorization': 'Bearer ' + authToken,
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(config)
        });

        if (!configResponse.ok) throw new Error('Failed to save config');

        // Save secrets
        const secrets = {};
        form.querySelectorAll('[data-secret-key]').forEach(input => {
            const key = input.getAttribute('data-secret-key');
            const value = input.value;
            if (value) {
                secrets[key] = value;
            }
        });

        // Save each secret individually
        const secretPromises = Object.entries(secrets).map(([key, value]) =>
            fetch(`/api/secrets/${encodeURIComponent(key)}`, {
                method: 'PUT',
                headers: {
                    'Authorization': 'Bearer ' + authToken,
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ value })
            })
        );

        if (secretPromises.length > 0) {
            const secretResults = await Promise.all(secretPromises);
            const failedSecrets = secretResults.filter(r => !r.ok);

            if (failedSecrets.length > 0) {
                throw new Error(`Failed to save ${failedSecrets.length} secret(s)`);
            }

            alert(`✓ Configuration and ${Object.keys(secrets).length} secret(s) for "${pluginName}" saved successfully!\n\nPlease restart the drakeify proxy for changes to take effect.`);
        } else {
            alert(`✓ Configuration for "${pluginName}" saved successfully!\n\nPlease restart the drakeify proxy for changes to take effect.`);
        }

        // Close modal
        form.closest('div').parentElement.remove();

    } catch (err) {
        console.error('Error saving plugin config:', err);
        alert(`Failed to save configuration: ${err.message}`);
    }
}


