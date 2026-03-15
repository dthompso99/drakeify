// Session Monitoring Module

let currentAccountId = '';
let allSessions = [];
let autoRefreshInterval = null;
let currentSessionData = null;

/**
 * Load sessions for a given account ID
 */
function loadSessions() {
    const accountId = document.getElementById('account-id-input').value.trim();
    if (!accountId) {
        alert('Please enter an Account ID');
        return;
    }

    currentAccountId = accountId;
    refreshSessions();
    
    // Show filter input
    document.getElementById('sessions-filter').style.display = 'block';
}

/**
 * Toggle auto-refresh functionality
 */
function toggleAutoRefresh() {
    const checkbox = document.getElementById('auto-refresh-checkbox');
    
    if (checkbox.checked) {
        // Start auto-refresh
        autoRefreshInterval = setInterval(() => {
            if (currentAccountId) {
                refreshSessions();
            }
        }, 30000); // 30 seconds
    } else {
        // Stop auto-refresh
        if (autoRefreshInterval) {
            clearInterval(autoRefreshInterval);
            autoRefreshInterval = null;
        }
    }
}

/**
 * Refresh the sessions list
 */
function refreshSessions() {
    if (!currentAccountId) {
        return;
    }

    if (!authToken) {
        const sessionsList = document.getElementById('sessions-list');
        if (sessionsList) {
            sessionsList.innerHTML = '<div class="error">Please authenticate first</div>';
        }
        return;
    }

    const sessionsList = document.getElementById('sessions-list');
    if (!sessionsList) {
        // Sessions view is not currently loaded, stop auto-refresh
        if (autoRefreshInterval) {
            clearInterval(autoRefreshInterval);
            autoRefreshInterval = null;
        }
        return;
    }

    sessionsList.innerHTML = '<div class="loading">Loading sessions...</div>';

    fetch(`/api/sessions?account_id=${encodeURIComponent(currentAccountId)}`, {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) {
            if (r.status === 400) {
                throw new Error('Invalid account ID');
            }
            throw new Error('Failed to load sessions');
        }
        return r.json();
    })
    .then(sessions => {
        allSessions = sessions; // Store for filtering
        const list = document.getElementById('sessions-list');
        if (sessions.length === 0) {
            list.innerHTML = `<div class="empty-state">No sessions found for account: ${currentAccountId}</div>`;
            document.getElementById('sessions-filter').style.display = 'none';
        } else {
            renderSessionsList(sessions);
            document.getElementById('sessions-filter').style.display = 'block';
        }
    })
    .catch(err => {
        console.error('Error loading sessions:', err);
        document.getElementById('sessions-list').innerHTML =
            `<div class="error">Failed to load sessions: ${err.message}</div>`;
    });
}

/**
 * Filter sessions based on search input
 */
function filterSessions() {
    const searchTerm = document.getElementById('session-search').value.toLowerCase().trim();
    
    if (!searchTerm) {
        // Show all sessions
        renderSessionsList(allSessions);
        return;
    }

    // Filter sessions
    const filtered = allSessions.filter(session => {
        const sessionId = (session.session_id || '').toLowerCase();
        const title = ((session.metadata && session.metadata.title) || '').toLowerCase();
        const tags = (session.metadata && session.metadata.tags) || [];
        const tagsStr = tags.join(' ').toLowerCase();

        return sessionId.includes(searchTerm) || 
               title.includes(searchTerm) || 
               tagsStr.includes(searchTerm);
    });

    renderSessionsList(filtered);
}

/**
 * Render the sessions list table
 * @param {Array} sessions - Array of session objects
 */
function renderSessionsList(sessions) {
    const list = document.getElementById('sessions-list');
    
    // Sort sessions by updated_at (most recent first)
    sessions.sort((a, b) => new Date(b.updated_at) - new Date(a.updated_at));

    // Calculate statistics
    const totalMessages = sessions.reduce((sum, s) => sum + (s.messages || []).length, 0);
    const avgMessages = sessions.length > 0 ? (totalMessages / sessions.length).toFixed(1) : 0;
    const allTags = new Set();
    sessions.forEach(s => {
        (s.metadata?.tags || []).forEach(tag => allTags.add(tag));
    });

    let html = `
        <div style="display: grid; grid-template-columns: repeat(4, 1fr); gap: 1rem; margin-bottom: 1.5rem;">
            <div style="background: #0f172a; padding: 1rem; border-radius: 8px; border-left: 3px solid #3b82f6;">
                <div style="color: #94a3b8; font-size: 0.75rem; text-transform: uppercase; margin-bottom: 0.25rem;">Total Sessions</div>
                <div style="color: #e2e8f0; font-size: 1.5rem; font-weight: 600;">${sessions.length}</div>
            </div>
            <div style="background: #0f172a; padding: 1rem; border-radius: 8px; border-left: 3px solid #10b981;">
                <div style="color: #94a3b8; font-size: 0.75rem; text-transform: uppercase; margin-bottom: 0.25rem;">Total Messages</div>
                <div style="color: #e2e8f0; font-size: 1.5rem; font-weight: 600;">${totalMessages}</div>
            </div>
            <div style="background: #0f172a; padding: 1rem; border-radius: 8px; border-left: 3px solid #f59e0b;">
                <div style="color: #94a3b8; font-size: 0.75rem; text-transform: uppercase; margin-bottom: 0.25rem;">Avg Messages/Session</div>
                <div style="color: #e2e8f0; font-size: 1.5rem; font-weight: 600;">${avgMessages}</div>
            </div>
            <div style="background: #0f172a; padding: 1rem; border-radius: 8px; border-left: 3px solid #8b5cf6;">
                <div style="color: #94a3b8; font-size: 0.75rem; text-transform: uppercase; margin-bottom: 0.25rem;">Unique Tags</div>
                <div style="color: #e2e8f0; font-size: 1.5rem; font-weight: 600;">${allTags.size}</div>
            </div>
        </div>
        <div style="margin-bottom: 1rem; color: #94a3b8;">
            Showing ${sessions.length} of ${allSessions.length} session${allSessions.length !== 1 ? 's' : ''} for account: <strong style="color: #e2e8f0;">${currentAccountId}</strong>
        </div>
        <table>
            <thead>
                <tr>
                    <th>Session ID</th>
                    <th>Title</th>
                    <th>Messages</th>
                    <th>Tags</th>
                    <th>Created</th>
                    <th>Updated</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
    `;

    sessions.forEach(session => {
        const messages = session.messages || [];
        const metadata = session.metadata || {};
        const title = metadata.title || '<em style="color: #64748b;">Untitled</em>';
        const tags = metadata.tags || [];
        const tagsHtml = tags.length > 0
            ? tags.map(tag => `<span class="badge" style="background: #3b82f6; margin-right: 0.25rem;">${tag}</span>`).join('')
            : '<span style="color: #64748b;">—</span>';

        html += `
            <tr>
                <td><code style="color: #60a5fa; font-size: 0.875rem;">${session.session_id}</code></td>
                <td>${title}</td>
                <td><span class="badge" style="background: #64748b;">${messages.length}</span></td>
                <td>${tagsHtml}</td>
                <td style="color: #94a3b8; font-size: 0.875rem;">${formatDate(session.created_at)}</td>
                <td style="color: #94a3b8; font-size: 0.875rem;">${formatDate(session.updated_at)}</td>
                <td>
                    <button onclick="viewSessionDetail('${session.session_id}')" class="btn btn-sm" style="background: #3b82f6;">View</button>
                </td>
            </tr>
        `;
    });

    html += '</tbody></table>';
    list.innerHTML = html;
}

/**
 * View detailed information about a session
 * @param {string} sessionId - The session ID to view
 */
function viewSessionDetail(sessionId) {
    if (!authToken || !currentAccountId) return;

    document.getElementById('session-detail-modal').style.display = 'block';
    document.getElementById('session-detail-title').textContent = `Session: ${sessionId}`;
    document.getElementById('session-detail-content').innerHTML = '<div class="loading">Loading session details...</div>';

    fetch(`/api/sessions/${sessionId}?account_id=${encodeURIComponent(currentAccountId)}`, {
        headers: { 'Authorization': 'Bearer ' + authToken }
    })
    .then(r => {
        if (!r.ok) throw new Error('Failed to load session details');
        return r.json();
    })
    .then(session => {
        currentSessionData = session; // Store for export
        renderSessionDetail(session);
    })
    .catch(err => {
        console.error('Error loading session details:', err);
        document.getElementById('session-detail-content').innerHTML =
            `<div class="error">Failed to load session details: ${err.message}</div>`;
    });
}

/**
 * Render session detail view
 * @param {Object} session - Session object
 */
function renderSessionDetail(session) {
    const metadata = session.metadata || {};
    // Messages might be in session.messages or metadata.messages
    let messages = session.messages || metadata.messages || [];

    // Ensure messages is always an array
    if (!Array.isArray(messages)) {
        console.warn('Messages is not an array:', messages);
        messages = [];
    }

    let html = `
        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem; margin-bottom: 2rem;">
            <div>
                <h3 style="color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; margin-bottom: 0.5rem;">Session ID</h3>
                <code style="color: #60a5fa; font-size: 1rem;">${session.session_id}</code>
            </div>
            <div>
                <h3 style="color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; margin-bottom: 0.5rem;">Account ID</h3>
                <code style="color: #60a5fa; font-size: 1rem;">${session.account_id}</code>
            </div>
            <div>
                <h3 style="color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; margin-bottom: 0.5rem;">Title</h3>
                <div style="color: #e2e8f0;">${metadata.title || '<em style="color: #64748b;">Untitled</em>'}</div>
            </div>
            <div>
                <h3 style="color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; margin-bottom: 0.5rem;">Tags</h3>
                <div>${(metadata.tags || []).length > 0
                    ? metadata.tags.map(tag => `<span class="badge" style="background: #3b82f6; margin-right: 0.25rem;">${tag}</span>`).join('')
                    : '<span style="color: #64748b;">No tags</span>'
                }</div>
            </div>
            <div>
                <h3 style="color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; margin-bottom: 0.5rem;">Created</h3>
                <div style="color: #e2e8f0;">${formatDate(session.created_at)}</div>
            </div>
            <div>
                <h3 style="color: #94a3b8; font-size: 0.875rem; text-transform: uppercase; margin-bottom: 0.5rem;">Updated</h3>
                <div style="color: #e2e8f0;">${formatDate(session.updated_at)}</div>
            </div>
        </div>
    `;

    // Additional metadata fields
    const extraFields = Object.keys(metadata.extra || {});
    if (extraFields.length > 0) {
        html += `
            <div style="margin-bottom: 2rem; padding: 1rem; background: #0f172a; border-radius: 8px; border: 1px solid #334155;">
                <h3 style="color: #f1f5f9; margin-bottom: 1rem;">Additional Metadata</h3>
                <pre style="color: #94a3b8; overflow-x: auto; font-size: 0.875rem;">${JSON.stringify(metadata.extra, null, 2)}</pre>
            </div>
        `;
    }

    // Messages
    html += `
        <div style="border-top: 1px solid #334155; padding-top: 1.5rem;">
            <h3 style="color: #f1f5f9; margin-bottom: 1rem;">Messages (${messages.length})</h3>
    `;

    if (messages.length === 0) {
        html += '<div class="empty-state">No messages in this session</div>';
    } else {
        messages.forEach((msg, idx) => {
            const role = msg.role || 'unknown';
            const content = msg.content || '';
            const roleColor = role === 'user' ? '#10b981' : role === 'assistant' ? '#3b82f6' : '#64748b';
            const roleIcon = role === 'user' ? '👤' : role === 'assistant' ? '🤖' : '⚙️';

            html += `
                <div style="margin-bottom: 1rem; padding: 1rem; background: #0f172a; border-radius: 8px; border-left: 3px solid ${roleColor};">
                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem;">
                        <div style="display: flex; align-items: center; gap: 0.5rem;">
                            <span style="font-size: 1.25rem;">${roleIcon}</span>
                            <span class="badge" style="background: ${roleColor};">${role}</span>
                            <span style="color: #64748b; font-size: 0.875rem;">Message ${idx + 1}</span>
                        </div>
                        <button onclick="toggleMessageContent(${idx})" class="btn btn-sm" style="background: #475569;">
                            <span id="toggle-icon-${idx}">▼</span> Toggle
                        </button>
                    </div>
                    <div id="message-content-${idx}" style="color: #cbd5e1; white-space: pre-wrap; word-wrap: break-word; max-height: 300px; overflow-y: auto; font-family: 'Courier New', monospace; font-size: 0.875rem; line-height: 1.5;">
                        ${escapeHtml(typeof content === 'string' ? content : JSON.stringify(content, null, 2))}
                    </div>
                </div>
            `;
        });
    }

    html += '</div>';
    document.getElementById('session-detail-content').innerHTML = html;
}

/**
 * Toggle message content visibility
 * @param {number} idx - Message index
 */
function toggleMessageContent(idx) {
    const content = document.getElementById(`message-content-${idx}`);
    const icon = document.getElementById(`toggle-icon-${idx}`);

    if (content.style.display === 'none') {
        content.style.display = 'block';
        icon.textContent = '▼';
    } else {
        content.style.display = 'none';
        icon.textContent = '▶';
    }
}

/**
 * Close the session detail modal
 */
function closeSessionDetail() {
    document.getElementById('session-detail-modal').style.display = 'none';
}

/**
 * Export current session data as JSON
 */
function exportCurrentSession() {
    if (!currentSessionData) {
        alert('No session data to export');
        return;
    }

    const dataStr = JSON.stringify(currentSessionData, null, 2);
    const dataBlob = new Blob([dataStr], { type: 'application/json' });
    const url = URL.createObjectURL(dataBlob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `session-${currentSessionData.session_id}-${new Date().toISOString().split('T')[0]}.json`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
}

