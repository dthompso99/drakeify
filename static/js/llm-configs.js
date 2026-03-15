// LLM Configuration Management Module

/**
 * Load all LLM configurations
 */
function loadConfigs() {
    if (!authToken) {
        document.getElementById('config-list').innerHTML =
            '<div class="error">Please enter your authentication token above</div>';
        return;
    }

    fetch('/api/llm/configs', {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) {
            if (r.status === 401) {
                throw new Error('Authentication failed');
            }
            throw new Error('Request failed');
        }
        return r.json();
    })
    .then(configs => {
        const list = document.getElementById('config-list');
        if (configs.length === 0) {
            list.innerHTML = '<div class="empty-state">No configurations found. Click "Add Configuration" to create one.</div>';
        } else {
            list.innerHTML = '<table><thead><tr><th>Name</th><th>Model</th><th>Host</th><th>Context Size</th><th>Priority</th><th>Status</th><th>Actions</th></tr></thead><tbody>' +
                configs.map(c => `<tr>
                    <td><strong>${c.name}</strong></td>
                    <td>${c.model}</td>
                    <td>${c.host}</td>
                    <td>${(c.context_size || 32768).toLocaleString()} tokens</td>
                    <td>${c.priority}</td>
                    <td><span class="badge badge-${c.enabled ? 'enabled' : 'disabled'}">${c.enabled ? 'Enabled' : 'Disabled'}</span></td>
                    <td>
                        <button onclick="editConfig('${c.id}')" class="btn btn-sm" style="background: #3b82f6; margin-right: 0.25rem;">Edit</button>
                        <button onclick="deleteConfig('${c.id}', '${c.name.replace(/'/g, "\\'")}')" class="btn btn-sm" style="background: #ef4444;">Delete</button>
                    </td>
                </tr>`).join('') +
                '</tbody></table>';
        }
    })
    .catch(err => {
        console.error('Load configs error:', err);
        const configList = document.getElementById('config-list');
        if (configList) {
            // Only clear auth token if this is actually an authentication error
            if (err.message === 'Authentication failed') {
                localStorage.removeItem('drakeify_token');
                authToken = null;
                configList.innerHTML =
                    '<div class="error">Authentication failed. Please enter your token again.</div>';
                showLoginForm();
            } else {
                configList.innerHTML =
                    '<div class="error">Failed to load configurations. Please try again.</div>';
            }
        }
    });
}

/**
 * Show the add configuration form
 */
function showAddConfigForm() {
    document.getElementById('form-title').textContent = 'Add LLM Configuration';
    document.getElementById('config-form').reset();
    document.getElementById('config-id').value = '';
    document.getElementById('config-enabled').checked = true;
    document.getElementById('config-priority').value = '100';
    document.getElementById('config-context-size').value = '32768';
    document.getElementById('config-timeout').value = '900';
    document.getElementById('config-form-modal').style.display = 'block';
}

/**
 * Hide the configuration form modal
 */
function hideConfigForm() {
    document.getElementById('config-form-modal').style.display = 'none';
}

/**
 * Edit an existing configuration
 * @param {string} id - Configuration ID
 */
function editConfig(id) {
    // Fetch the config details
    fetch(`/api/llm/configs/${id}`, {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => r.json())
    .then(config => {
        document.getElementById('form-title').textContent = 'Edit LLM Configuration';
        document.getElementById('config-id').value = config.id;
        document.getElementById('config-name').value = config.name;
        document.getElementById('config-model').value = config.model;
        document.getElementById('config-host').value = config.host;
        document.getElementById('config-context-size').value = config.context_size || 32768;
        document.getElementById('config-timeout').value = config.timeout_secs || 900;
        document.getElementById('config-priority').value = config.priority;
        document.getElementById('config-enabled').checked = config.enabled;
        document.getElementById('config-api-key').value = ''; // Don't show existing key
        document.getElementById('config-api-key').placeholder = 'Leave empty to keep existing';
        document.getElementById('config-form-modal').style.display = 'block';
    })
    .catch(err => {
        console.error('Error loading config:', err);
        alert('Failed to load configuration');
    });
}

/**
 * Save a configuration (create or update)
 * @param {Event} event - Form submit event
 */
function saveConfig(event) {
    event.preventDefault();

    const id = document.getElementById('config-id').value;
    const name = document.getElementById('config-name').value;

    const data = {
        id: id || name.toLowerCase().replace(/[^a-z0-9]+/g, '-'), // Generate ID from name if creating
        name: name,
        model: document.getElementById('config-model').value,
        host: document.getElementById('config-host').value,
        endpoint: '/v1/chat/completions', // Default endpoint
        context_size: parseInt(document.getElementById('config-context-size').value),
        timeout_secs: parseInt(document.getElementById('config-timeout').value),
        priority: parseInt(document.getElementById('config-priority').value),
        enabled: document.getElementById('config-enabled').checked
    };

    // Only include account_id (API key) if it's been entered
    const apiKey = document.getElementById('config-api-key').value;
    if (apiKey) {
        data.account_id = apiKey;
    }

    const url = id ? `/api/llm/configs/${id}` : '/api/llm/configs';
    const method = id ? 'PUT' : 'POST';

    fetch(url, {
        method: method,
        headers: {
            'Authorization': 'Bearer ' + authToken,
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(data)
    })
    .then(r => {
        if (!r.ok) throw new Error('Save failed');
        return r.json();
    })
    .then(() => {
        hideConfigForm();
        loadConfigs();
    })
    .catch(err => {
        console.error('Error saving config:', err);
        alert('Failed to save configuration');
    });
}

/**
 * Delete a configuration
 * @param {string} id - Configuration ID
 * @param {string} name - Configuration name (for confirmation)
 */
function deleteConfig(id, name) {
    if (!confirm(`Are you sure you want to delete "${name}"?`)) {
        return;
    }

    fetch(`/api/llm/configs/${id}`, {
        method: 'DELETE',
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Delete failed');
        loadConfigs();
    })
    .catch(err => {
        console.error('Error deleting config:', err);
        alert('Failed to delete configuration');
    });
}

