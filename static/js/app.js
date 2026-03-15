// Drakeify Admin - Main Application
// Global auth token
let authToken = localStorage.getItem('drakeify_token');

// ============================================================================
// VIEW REGISTRY - Plugin-ready architecture
// ============================================================================
// This registry will eventually be fetched from /api/plugins/manifest
// For now, it's hardcoded with built-in views

const viewRegistry = [
    {
        id: 'llm-configs',
        name: 'LLM Configurations',
        section: 'Configuration',
        loader: loadLlmConfigsView
    },
    {
        id: 'plugins',
        name: 'Plugin Manager',
        section: 'Configuration',
        loader: loadPluginManagerView
    },
    {
        id: 'tools',
        name: 'Tool Manager',
        section: 'Configuration',
        loader: loadToolManagerView
    },
    {
        id: 'routes',
        name: 'Routes',
        section: 'Configuration',
        loader: loadPlaceholderView
    },
    {
        id: 'middleware',
        name: 'Middleware',
        section: 'Configuration',
        loader: loadPlaceholderView
    },
    {
        id: 'metrics',
        name: 'Metrics',
        section: 'Monitoring',
        loader: loadPlaceholderView
    },
    {
        id: 'logs',
        name: 'Logs',
        section: 'Monitoring',
        loader: loadPlaceholderView
    },
    {
        id: 'session-logs',
        name: 'Session Logs',
        section: 'Monitoring',
        loader: loadSessionLogsView
    },
    {
        id: 'settings',
        name: 'Settings',
        section: 'System',
        loader: loadPlaceholderView
    }
];

// ============================================================================
// NAVIGATION BUILDER
// ============================================================================

function buildNavigation() {
    const navMenu = document.getElementById('nav-menu');

    // Group views by section
    const sections = {};
    viewRegistry.forEach(view => {
        if (!sections[view.section]) {
            sections[view.section] = [];
        }
        sections[view.section].push(view);
    });

    // Build HTML for each section
    let html = '';
    Object.keys(sections).forEach(sectionName => {
        html += `<div class="nav-section">`;
        html += `<div class="nav-section-title">${sectionName}</div>`;

        sections[sectionName].forEach(view => {
            html += `<a class="nav-item" data-view="${view.id}">${view.name}</a>`;
        });

        html += `</div>`;
    });

    navMenu.innerHTML = html;

    // Attach click handlers
    document.querySelectorAll('.nav-item').forEach(item => {
        item.addEventListener('click', function(e) {
            e.preventDefault();
            const viewId = this.getAttribute('data-view');
            loadView(viewId);
        });
    });

    // Set first view as active
    if (viewRegistry.length > 0) {
        loadView(viewRegistry[0].id);
    }
}

// ============================================================================
// VIEW LOADER
// ============================================================================

function loadView(viewId) {
    const view = viewRegistry.find(v => v.id === viewId);
    if (!view) {
        console.error('View not found:', viewId);
        return;
    }

    // Update active nav item
    document.querySelectorAll('.nav-item').forEach(item => {
        if (item.getAttribute('data-view') === viewId) {
            item.classList.add('active');
        } else {
            item.classList.remove('active');
        }
    });

    // Update page title
    document.getElementById('page-title').textContent = view.name;

    // Load view content
    if (view.loader) {
        view.loader(viewId);
    }
}

// ============================================================================
// VIEW LOADERS - Each view has its own loader function
// ============================================================================

function loadPlaceholderView(viewId) {
    const view = viewRegistry.find(v => v.id === viewId);
    const container = document.getElementById('view-container');
    container.innerHTML = `
        <div class="card">
            <h2>${view.name}</h2>
            <p>${view.name} interface coming soon...</p>
        </div>
    `;
}

function loadLlmConfigsView(viewId) {
    const container = document.getElementById('view-container');
    container.innerHTML = `
        <div class="card">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                <h2 style="margin: 0;">LLM Configurations</h2>
                <button onclick="showAddConfigForm()" class="btn btn-primary">+ Add Configuration</button>
            </div>
            <div id="config-list">
                <div class="loading">Loading configurations...</div>
            </div>
        </div>

        <!-- Add/Edit Form Modal (hidden by default) -->
        <div id="config-form-modal" style="display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.7); z-index: 1000; padding: 2rem; overflow-y: auto;">
            <div style="max-width: 600px; margin: 0 auto; background: #1e293b; border-radius: 12px; padding: 2rem; box-shadow: 0 20px 25px -5px rgba(0,0,0,0.5);">
                <h2 id="form-title" style="margin-bottom: 1.5rem;">Add LLM Configuration</h2>
                <form id="config-form" onsubmit="saveConfig(event)">
                    <input type="hidden" id="config-id" value="">

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Name *</label>
                        <input type="text" id="config-name" required
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                    </div>

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Model *</label>
                        <input type="text" id="config-model" required placeholder="e.g., gpt-4, claude-3-opus"
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                    </div>

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Host *</label>
                        <input type="text" id="config-host" required placeholder="e.g., https://api.openai.com"
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                    </div>

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">API Key</label>
                        <input type="password" id="config-api-key" placeholder="Leave empty to keep existing"
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                    </div>

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Context Size *</label>
                        <input type="number" id="config-context-size" required value="32768" min="1024" max="1048576"
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                        <small style="color: #94a3b8;">Maximum context window in tokens (e.g., 32768, 128000, 262144)</small>
                    </div>

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Timeout (seconds) *</label>
                        <input type="number" id="config-timeout" required value="900" min="10" max="3600"
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                        <small style="color: #94a3b8;">Request timeout in seconds (default: 900 = 15 minutes)</small>
                    </div>

                    <div style="margin-bottom: 1rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Priority *</label>
                        <input type="number" id="config-priority" required value="100" min="0"
                               style="width: 100%; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
                        <small style="color: #94a3b8;">Lower numbers = higher priority</small>
                    </div>

                    <div style="margin-bottom: 1.5rem;">
                        <label style="display: flex; align-items: center; cursor: pointer;">
                            <input type="checkbox" id="config-enabled" checked
                                   style="margin-right: 0.5rem; width: 1.25rem; height: 1.25rem;">
                            <span>Enabled</span>
                        </label>
                    </div>

                    <div style="display: flex; gap: 0.5rem; justify-content: flex-end;">
                        <button type="button" onclick="hideConfigForm()" class="btn" style="background: #475569;">Cancel</button>
                        <button type="submit" class="btn btn-primary">Save</button>
                    </div>
                </form>
            </div>
        </div>
    `;

    // Load the actual data
    loadConfigs();
}

function loadPluginManagerView(viewId) {
    const container = document.getElementById('view-container');
    container.innerHTML = `
        <div class="card">
            <h2>Plugin Manager</h2>

            <!-- Tabs -->
            <div style="display: flex; gap: 1rem; margin-bottom: 1.5rem; border-bottom: 2px solid #334155;">
                <button id="tab-installed" onclick="showPluginTab('installed')" class="plugin-tab active" style="padding: 0.75rem 1.5rem; background: none; border: none; border-bottom: 2px solid #3b82f6; color: #3b82f6; font-weight: 600; cursor: pointer; margin-bottom: -2px;">
                    Installed
                </button>
                <button id="tab-available" onclick="showPluginTab('available')" class="plugin-tab" style="padding: 0.75rem 1.5rem; background: none; border: none; color: #94a3b8; font-weight: 600; cursor: pointer; margin-bottom: -2px;">
                    Available
                </button>
            </div>

            <!-- Installed Plugins Tab -->
            <div id="installed-plugins-tab" class="plugin-tab-content">
                <div id="installed-plugins-list">
                    <div class="loading">Loading installed plugins...</div>
                </div>
            </div>

            <!-- Available Plugins Tab -->
            <div id="available-plugins-tab" class="plugin-tab-content" style="display: none;">
                <div id="available-plugins-list">
                    <div class="loading">Loading available plugins...</div>
                </div>
            </div>
        </div>
    `;

    // Load all tabs
    loadInstalledPlugins();
    loadAvailablePlugins();
}

function loadSessionLogsView(viewId) {
    const container = document.getElementById('view-container');
    container.innerHTML = `
        <div class="card">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                <h2 style="margin: 0;">Session Monitoring</h2>
                <div style="display: flex; gap: 1rem; align-items: center;">
                    <input type="text" id="account-id-input" placeholder="Enter Account ID"
                           style="padding: 0.5rem 1rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0; min-width: 250px;">
                    <button onclick="loadSessions()" class="btn btn-primary">Load Sessions</button>
                    <button onclick="refreshSessions()" class="btn" style="background: #64748b;">↻ Refresh</button>
                    <label style="display: flex; align-items: center; gap: 0.5rem; color: #94a3b8; cursor: pointer;">
                        <input type="checkbox" id="auto-refresh-checkbox" onchange="toggleAutoRefresh()"
                               style="width: 1.25rem; height: 1.25rem; cursor: pointer;">
                        <span style="font-size: 0.875rem;">Auto-refresh (30s)</span>
                    </label>
                </div>
            </div>

            <div id="sessions-filter" style="display: none; margin-bottom: 1rem;">
                <input type="text" id="session-search" placeholder="Search sessions by ID, title, or tags..."
                       oninput="filterSessions()"
                       style="width: 100%; padding: 0.75rem 1rem; border: 1px solid #475569; border-radius: 6px; background: #0f172a; color: #e2e8f0;">
            </div>

            <div id="sessions-list">
                <div class="empty-state">Enter an Account ID and click "Load Sessions" to view session data</div>
            </div>
        </div>

        <!-- Session Detail Modal -->
        <div id="session-detail-modal" style="display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.8); z-index: 1000; padding: 2rem; overflow-y: auto;">
            <div style="max-width: 1200px; margin: 0 auto; background: #1e293b; border-radius: 12px; padding: 2rem; box-shadow: 0 20px 25px -5px rgba(0,0,0,0.5);">
                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1.5rem;">
                    <h2 id="session-detail-title" style="margin: 0;">Session Details</h2>
                    <div style="display: flex; gap: 0.5rem;">
                        <button onclick="exportCurrentSession()" class="btn" style="background: #10b981;">⬇ Export JSON</button>
                        <button onclick="closeSessionDetail()" class="btn" style="background: #64748b;">✕ Close</button>
                    </div>
                </div>
                <div id="session-detail-content">
                    <!-- Session details will be loaded here -->
                </div>
            </div>
        </div>
    `;
}

function loadToolManagerView() {
    const container = document.getElementById('view-container');
    container.innerHTML = `
        <div class="view-header">
            <h2>🛠️ Tool Manager</h2>
            <p style="color: #94a3b8;">Manage tools for LLM function calling</p>
        </div>

        <div class="plugin-manager">
            <div class="plugin-tabs" style="border-bottom: 2px solid #334155; margin-bottom: 2rem;">
                <button id="tab-installed-tools" onclick="showToolTab('installed')" class="tool-tab active" style="padding: 0.75rem 1.5rem; background: none; border: none; color: #3b82f6; font-weight: 600; cursor: pointer; border-bottom: 2px solid #3b82f6; margin-bottom: -2px;">
                    Installed
                </button>
                <button id="tab-available-tools" onclick="showToolTab('available')" class="tool-tab" style="padding: 0.75rem 1.5rem; background: none; border: none; color: #94a3b8; font-weight: 600; cursor: pointer; margin-bottom: -2px;">
                    Available
                </button>
                <button id="tab-publish-tools" onclick="showToolTab('publish')" class="tool-tab" style="padding: 0.75rem 1.5rem; background: none; border: none; color: #94a3b8; font-weight: 600; cursor: pointer; margin-bottom: -2px;">
                    Publish
                </button>
            </div>

            <!-- Installed Tools Tab -->
            <div id="installed-tools-tab" class="tool-tab-content">
                <div id="installed-tools-list">
                    <div class="loading">Loading installed tools...</div>
                </div>
            </div>

            <!-- Available Tools Tab -->
            <div id="available-tools-tab" class="tool-tab-content" style="display: none;">
                <div id="available-tools-list">
                    <div class="loading">Loading available tools...</div>
                </div>
            </div>

            <!-- Publish Tools Tab -->
            <div id="publish-tools-tab" class="tool-tab-content" style="display: none;">
                <div id="publish-tools-list">
                    <div class="loading">Loading unpublished tools...</div>
                </div>
            </div>
        </div>
    `;

    // Load all tabs
    loadInstalledTools();
    loadAvailableTools();
    loadUnpublishedTools();
}

function showToolTab(tabName) {
    // Update tab buttons
    document.querySelectorAll('.tool-tab').forEach(tab => {
        tab.style.borderBottom = 'none';
        tab.style.color = '#94a3b8';
    });
    document.getElementById(`tab-${tabName}-tools`).style.borderBottom = '2px solid #3b82f6';
    document.getElementById(`tab-${tabName}-tools`).style.color = '#3b82f6';

    // Show/hide tab content
    document.querySelectorAll('.tool-tab-content').forEach(content => {
        content.style.display = 'none';
    });
    document.getElementById(`${tabName}-tools-tab`).style.display = 'block';
}

// ============================================================================
// AUTHENTICATION
// ============================================================================

function showLoginForm() {
    const existingForm = document.getElementById('login-form');
    if (existingForm) return; // Already showing

    const loginForm = document.createElement('div');
    loginForm.id = 'login-form';
    loginForm.style.cssText = 'margin-top: 1rem; padding: 1rem; background: #334155; border-radius: 8px;';
    loginForm.innerHTML = `
        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">Authentication Token:</label>
        <div style="display: flex; gap: 0.5rem;">
            <input type="password" id="token-input" placeholder="Enter your token"
                   style="flex: 1; padding: 0.5rem; border: 1px solid #475569; border-radius: 6px; background: #1e293b; color: #e2e8f0; font-family: monospace;">
            <button onclick="submitToken()" class="btn btn-primary">Login</button>
        </div>
    `;

    // Insert at the top of the content area
    const viewContainer = document.getElementById('view-container');
    viewContainer.insertBefore(loginForm, viewContainer.firstChild);
    document.getElementById('token-input').focus();

    // Allow Enter key to submit
    document.getElementById('token-input').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') submitToken();
    });
}

function submitToken() {
    const input = document.getElementById('token-input');
    const token = input.value.trim();
    if (token) {
        authToken = token;
        localStorage.setItem('drakeify_token', token);
        document.getElementById('login-form').remove();
        loadConfigs();
        connectWebSocket(); // Connect WebSocket after authentication
    }
}

// ============================================================================
// WEBSOCKET CONNECTION
// ============================================================================

let ws = null;
const statusEl = document.getElementById('ws-status');

function connectWebSocket() {
    const token = localStorage.getItem('drakeify_token');
    if (!token) {
        statusEl.textContent = '● Not authenticated';
        statusEl.style.background = '#64748b';
        return;
    }

    const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    // Include token as query parameter for WebSocket auth
    ws = new WebSocket(`${wsProtocol}//${window.location.host}/ws?token=${encodeURIComponent(token)}`);

    ws.onopen = () => {
        statusEl.textContent = '● Connected';
        statusEl.style.background = '#10b981';
    };

    ws.onclose = () => {
        statusEl.textContent = '● Disconnected';
        statusEl.style.background = '#ef4444';
        // Try to reconnect after 5 seconds
        setTimeout(connectWebSocket, 5000);
    };

    ws.onerror = () => {
        statusEl.textContent = '● Error';
        statusEl.style.background = '#f59e0b';
    };

    ws.onmessage = (event) => {
        const update = JSON.parse(event.data);
        console.log('WebSocket update:', update);

        // Refresh the appropriate list based on update type
        switch (update.type) {
            case 'LlmConfigCreated':
            case 'LlmConfigUpdated':
            case 'LlmConfigDeleted':
                // Only reload configs if we're on the LLM config page
                if (document.getElementById('config-list')) {
                    loadConfigs();
                }
                break;
            case 'PluginInstalled':
            case 'PluginUninstalled':
                // Reload plugin lists if they exist
                if (typeof loadInstalledPlugins === 'function') {
                    loadInstalledPlugins();
                }
                if (typeof loadAvailablePlugins === 'function') {
                    loadAvailablePlugins();
                }
                break;
            case 'ToolInstalled':
            case 'ToolUninstalled':
                // Reload tool lists if they exist
                if (typeof loadInstalledTools === 'function') {
                    loadInstalledTools();
                }
                if (typeof loadAvailableTools === 'function') {
                    loadAvailableTools();
                }
                break;
        }
    };
}

// ============================================================================
// INITIALIZATION
// ============================================================================

// Wait for DOM to be ready before initializing
document.addEventListener('DOMContentLoaded', function() {
    // Build navigation from registry
    buildNavigation();

    // Handle authentication
    if (!authToken) {
        showLoginForm();
    } else {
        connectWebSocket(); // Connect WebSocket if already authenticated
    }
});

