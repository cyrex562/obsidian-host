// Auth module - handles authentication UI, session management, and admin dashboard
(function () {
    'use strict';

    let currentUser = null;
    let allUsers = [];

    // =========================================================================
    // Auth initialization
    // =========================================================================

    async function initAuth() {
        try {
            const statusResp = await fetch('/api/auth/status');
            const statusData = await statusResp.json();

            if (!statusData.auth_enabled) {
                return;
            }

            const meResp = await fetch('/api/auth/me');

            if (meResp.status === 401) {
                window.location.href = '/login.html?status=unauthorized';
                return;
            }

            if (meResp.status === 403) {
                window.location.href = '/login.html?status=pending';
                return;
            }

            if (!meResp.ok) {
                console.error('Auth check failed:', meResp.status);
                return;
            }

            currentUser = await meResp.json();

            // Show logout button
            const logoutBtn = document.getElementById('logout-btn');
            if (logoutBtn) {
                logoutBtn.classList.remove('hidden');
                logoutBtn.title = `Logout (${currentUser.email})`;
                logoutBtn.addEventListener('click', handleLogout);
            }

            // Show admin button if admin
            if (currentUser.role === 'admin') {
                const adminBtn = document.getElementById('admin-panel-btn');
                if (adminBtn) {
                    adminBtn.classList.remove('hidden');
                    adminBtn.addEventListener('click', openAdminPanel);
                }
            }
        } catch (e) {
            console.error('Auth initialization failed:', e);
        }
    }

    async function handleLogout() {
        try {
            await fetch('/api/auth/logout', { method: 'POST' });
        } catch (e) {
            console.error('Logout request failed:', e);
        }
        window.location.href = '/login.html';
    }

    // =========================================================================
    // Admin Dashboard
    // =========================================================================

    async function openAdminPanel() {
        const modal = document.getElementById('admin-panel-modal');
        if (!modal) return;
        modal.classList.remove('hidden');
        initAdminTabs();
        await loadUsers();
    }

    function initAdminTabs() {
        const tabs = document.querySelectorAll('.admin-tab-btn');
        tabs.forEach(tab => {
            tab.addEventListener('click', () => {
                const tabName = tab.dataset.adminTab;
                tabs.forEach(t => t.classList.remove('active'));
                tab.classList.add('active');

                document.querySelectorAll('.admin-tab-content').forEach(c => c.classList.add('hidden'));
                const target = document.getElementById('admin-tab-' + tabName);
                if (target) target.classList.remove('hidden');
            });
        });

        // Search and filter for all-users tab
        const searchInput = document.getElementById('admin-user-search');
        const roleFilter = document.getElementById('admin-role-filter');

        if (searchInput) {
            searchInput.addEventListener('input', () => renderFilteredUsers());
        }
        if (roleFilter) {
            roleFilter.addEventListener('change', () => renderFilteredUsers());
        }
    }

    async function loadUsers() {
        try {
            const resp = await fetch('/api/admin/users');
            if (!resp.ok) {
                showError('admin-pending-list', 'Failed to load users.');
                showError('admin-all-list', 'Failed to load users.');
                return;
            }

            allUsers = await resp.json();
            updateStats();
            renderPendingList();
            renderFilteredUsers();
        } catch (e) {
            console.error('Failed to load users:', e);
            showError('admin-pending-list', 'Error loading users.');
            showError('admin-all-list', 'Error loading users.');
        }
    }

    function showError(containerId, message) {
        const el = document.getElementById(containerId);
        if (el) el.innerHTML = `<p style="color: var(--text-secondary);">${escapeHtml(message)}</p>`;
    }

    // =========================================================================
    // Stats
    // =========================================================================

    function updateStats() {
        const total = allUsers.length;
        const pending = allUsers.filter(u => u.role === 'pending').length;
        const active = allUsers.filter(u => u.role === 'admin' || u.role === 'user').length;
        const suspended = allUsers.filter(u => u.role === 'suspended').length;

        setStatValue('stat-total', total);
        setStatValue('stat-pending', pending);
        setStatValue('stat-active', active);
        setStatValue('stat-suspended', suspended);

        // Update badge
        const badge = document.getElementById('pending-badge');
        if (badge) {
            badge.textContent = pending;
            badge.classList.toggle('badge-zero', pending === 0);
        }
    }

    function setStatValue(id, value) {
        const el = document.getElementById(id);
        if (el) el.textContent = value;
    }

    // =========================================================================
    // Pending requests list
    // =========================================================================

    function renderPendingList() {
        const container = document.getElementById('admin-pending-list');
        if (!container) return;

        const pending = allUsers.filter(u => u.role === 'pending');

        if (pending.length === 0) {
            container.innerHTML = `
                <div class="admin-empty-state">
                    <div class="admin-empty-icon">&#10003;</div>
                    <p>No pending requests</p>
                </div>
            `;
            return;
        }

        container.innerHTML = pending.map(user => renderPendingCard(user)).join('');
        attachPendingHandlers(container);
    }

    function renderPendingCard(user) {
        const date = formatDate(user.created_at);
        return `
            <div class="admin-user-card card-pending" data-user-id="${escapeAttr(user.id)}">
                <div class="admin-user-info">
                    ${renderAvatar(user)}
                    <div class="admin-user-details">
                        <div class="admin-user-name">${escapeHtml(user.name)}</div>
                        <div class="admin-user-email">${escapeHtml(user.email)}</div>
                        <div class="admin-user-meta">Requested ${date}</div>
                    </div>
                </div>
                <div class="admin-user-actions">
                    <button class="admin-btn admin-btn-approve" data-action="approve" data-user-id="${escapeAttr(user.id)}">Approve</button>
                    <button class="admin-btn admin-btn-delete" data-action="reject" data-user-id="${escapeAttr(user.id)}">Reject</button>
                </div>
            </div>
        `;
    }

    function attachPendingHandlers(container) {
        container.querySelectorAll('[data-action="approve"]').forEach(btn => {
            btn.addEventListener('click', async () => {
                await updateRole(btn.dataset.userId, 'user');
            });
        });

        container.querySelectorAll('[data-action="reject"]').forEach(btn => {
            btn.addEventListener('click', async () => {
                const user = allUsers.find(u => u.id === btn.dataset.userId);
                const name = user ? user.name : 'this user';
                if (await confirmAction('Reject User', `Delete ${name}\'s account? This cannot be undone.`, 'Delete', true)) {
                    await deleteUser(btn.dataset.userId);
                }
            });
        });
    }

    // =========================================================================
    // All users list (with search/filter)
    // =========================================================================

    function renderFilteredUsers() {
        const container = document.getElementById('admin-all-list');
        if (!container) return;

        const search = (document.getElementById('admin-user-search')?.value || '').toLowerCase();
        const roleFilter = document.getElementById('admin-role-filter')?.value || 'all';

        let filtered = allUsers;

        if (roleFilter !== 'all') {
            filtered = filtered.filter(u => u.role === roleFilter);
        }

        if (search) {
            filtered = filtered.filter(u =>
                u.name.toLowerCase().includes(search) ||
                u.email.toLowerCase().includes(search)
            );
        }

        if (filtered.length === 0) {
            container.innerHTML = `
                <div class="admin-empty-state">
                    <p>No users match the current filters.</p>
                </div>
            `;
            return;
        }

        container.innerHTML = filtered.map(user => renderUserCard(user)).join('');
        attachUserHandlers(container);
    }

    function renderUserCard(user) {
        const isCurrentUser = currentUser && user.id === currentUser.id;
        const date = formatDate(user.created_at);
        const cardClass = user.role === 'pending' ? 'card-pending' :
                          user.role === 'suspended' ? 'card-suspended' : '';

        let actionsHtml = '';
        if (!isCurrentUser) {
            actionsHtml = renderUserActions(user);
        }

        return `
            <div class="admin-user-card ${cardClass}" data-user-id="${escapeAttr(user.id)}">
                <div class="admin-user-info">
                    ${renderAvatar(user)}
                    <div class="admin-user-details">
                        <div class="admin-user-name">${escapeHtml(user.name)}${isCurrentUser ? ' (you)' : ''}</div>
                        <div class="admin-user-email">${escapeHtml(user.email)}</div>
                        <div class="admin-user-meta">Joined ${date}</div>
                    </div>
                </div>
                <div class="admin-user-actions">
                    <span class="admin-role-badge role-${escapeAttr(user.role)}">${escapeHtml(user.role)}</span>
                    ${actionsHtml}
                </div>
            </div>
        `;
    }

    function renderUserActions(user) {
        const actions = [];

        switch (user.role) {
            case 'pending':
                actions.push(`<button class="admin-btn admin-btn-approve" data-action="approve" data-user-id="${escapeAttr(user.id)}">Approve</button>`);
                actions.push(`<button class="admin-btn admin-btn-delete" data-action="delete" data-user-id="${escapeAttr(user.id)}" title="Delete user">&#10005;</button>`);
                break;
            case 'user':
                actions.push(`<button class="admin-btn admin-btn-promote" data-action="promote" data-user-id="${escapeAttr(user.id)}">Promote</button>`);
                actions.push(`<button class="admin-btn admin-btn-suspend" data-action="suspend" data-user-id="${escapeAttr(user.id)}">Suspend</button>`);
                actions.push(`<button class="admin-btn admin-btn-delete" data-action="delete" data-user-id="${escapeAttr(user.id)}" title="Delete user">&#10005;</button>`);
                break;
            case 'admin':
                actions.push(`<button class="admin-btn admin-btn-demote" data-action="demote" data-user-id="${escapeAttr(user.id)}">Demote</button>`);
                break;
            case 'suspended':
                actions.push(`<button class="admin-btn admin-btn-unsuspend" data-action="unsuspend" data-user-id="${escapeAttr(user.id)}">Unsuspend</button>`);
                actions.push(`<button class="admin-btn admin-btn-delete" data-action="delete" data-user-id="${escapeAttr(user.id)}" title="Delete user">&#10005;</button>`);
                break;
        }

        return actions.join('');
    }

    function attachUserHandlers(container) {
        container.querySelectorAll('[data-action]').forEach(btn => {
            btn.addEventListener('click', async () => {
                const userId = btn.dataset.userId;
                const action = btn.dataset.action;
                const user = allUsers.find(u => u.id === userId);
                const name = user ? user.name : 'this user';

                switch (action) {
                    case 'approve':
                        await updateRole(userId, 'user');
                        break;
                    case 'promote':
                        if (await confirmAction('Promote to Admin', `Grant ${name} full admin privileges?`, 'Promote')) {
                            await updateRole(userId, 'admin');
                        }
                        break;
                    case 'demote':
                        if (await confirmAction('Demote to User', `Remove admin privileges from ${name}?`, 'Demote')) {
                            await updateRole(userId, 'user');
                        }
                        break;
                    case 'suspend':
                        if (await confirmAction('Suspend Account', `Suspend ${name}\'s account? They will be unable to access the application.`, 'Suspend', true)) {
                            await updateRole(userId, 'suspended');
                        }
                        break;
                    case 'unsuspend':
                        await updateRole(userId, 'user');
                        break;
                    case 'delete':
                        if (await confirmAction('Delete User', `Permanently delete ${name}\'s account? This cannot be undone.`, 'Delete', true)) {
                            await deleteUser(userId);
                        }
                        break;
                }
            });
        });
    }

    // =========================================================================
    // API calls
    // =========================================================================

    async function updateRole(userId, role) {
        try {
            const resp = await fetch(`/api/admin/users/${userId}/role`, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ role }),
            });

            if (!resp.ok) {
                const err = await resp.json().catch(() => ({ message: 'Unknown error' }));
                alert(`Failed to update role: ${err.message}`);
                return;
            }

            await loadUsers();
        } catch (e) {
            console.error('Failed to update user role:', e);
            alert('Failed to update user role');
        }
    }

    async function deleteUser(userId) {
        try {
            const resp = await fetch(`/api/admin/users/${userId}`, {
                method: 'DELETE',
            });

            if (!resp.ok && resp.status !== 204) {
                const err = await resp.json().catch(() => ({ message: 'Unknown error' }));
                alert(`Failed to delete user: ${err.message}`);
                return;
            }

            await loadUsers();
        } catch (e) {
            console.error('Failed to delete user:', e);
            alert('Failed to delete user');
        }
    }

    // =========================================================================
    // Confirm dialog
    // =========================================================================

    function confirmAction(title, message, confirmLabel, isDangerous) {
        return new Promise(resolve => {
            const overlay = document.createElement('div');
            overlay.className = 'admin-confirm-overlay';

            const dangerClass = isDangerous ? 'admin-btn-delete' : 'admin-btn-approve';

            overlay.innerHTML = `
                <div class="admin-confirm-dialog">
                    <div class="admin-confirm-title">${escapeHtml(title)}</div>
                    <div class="admin-confirm-message">${escapeHtml(message)}</div>
                    <div class="admin-confirm-actions">
                        <button class="admin-btn" id="confirm-cancel">Cancel</button>
                        <button class="admin-btn ${dangerClass}" id="confirm-ok">${escapeHtml(confirmLabel)}</button>
                    </div>
                </div>
            `;

            document.body.appendChild(overlay);

            overlay.querySelector('#confirm-cancel').addEventListener('click', () => {
                overlay.remove();
                resolve(false);
            });

            overlay.querySelector('#confirm-ok').addEventListener('click', () => {
                overlay.remove();
                resolve(true);
            });

            overlay.addEventListener('click', (e) => {
                if (e.target === overlay) {
                    overlay.remove();
                    resolve(false);
                }
            });
        });
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    function renderAvatar(user) {
        if (user.picture) {
            return `<img class="admin-user-avatar" src="${escapeAttr(user.picture)}" alt="" referrerpolicy="no-referrer">`;
        }
        const initials = (user.name || '?').charAt(0).toUpperCase();
        return `<div class="admin-user-avatar-placeholder">${initials}</div>`;
    }

    function formatDate(dateStr) {
        try {
            return new Date(dateStr).toLocaleDateString(undefined, {
                year: 'numeric',
                month: 'short',
                day: 'numeric'
            });
        } catch {
            return dateStr;
        }
    }

    function escapeHtml(str) {
        const div = document.createElement('div');
        div.textContent = str || '';
        return div.innerHTML;
    }

    function escapeAttr(str) {
        return (str || '').replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/'/g, '&#39;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }

    // =========================================================================
    // Modal close handler
    // =========================================================================

    document.addEventListener('click', (e) => {
        if (e.target.matches('[data-close-modal="admin-panel-modal"]')) {
            const modal = document.getElementById('admin-panel-modal');
            if (modal) modal.classList.add('hidden');
        }
    });

    // =========================================================================
    // Global 401 interceptor
    // =========================================================================

    const originalFetch = window.fetch;
    window.fetch = async function (...args) {
        const response = await originalFetch.apply(this, args);

        const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
        if (url.includes('/api/auth/')) {
            return response;
        }

        if (response.status === 401) {
            try {
                const statusResp = await originalFetch('/api/auth/status');
                const statusData = await statusResp.json();
                if (statusData.auth_enabled) {
                    window.location.href = '/login.html?status=expired';
                }
            } catch (e) {
                // Ignore
            }
        }

        return response;
    };

    // =========================================================================
    // Init
    // =========================================================================

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initAuth);
    } else {
        initAuth();
    }
})();
