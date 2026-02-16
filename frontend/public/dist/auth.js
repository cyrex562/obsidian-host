// Auth module - handles authentication UI and session management
(function () {
    'use strict';

    let currentUser = null;

    // Check auth status on page load
    async function initAuth() {
        try {
            const statusResp = await fetch('/api/auth/status');
            const statusData = await statusResp.json();

            if (!statusData.auth_enabled) {
                // Auth disabled - no UI changes needed
                return;
            }

            // Auth is enabled - check if user is logged in
            const meResp = await fetch('/api/auth/me');

            if (meResp.status === 401) {
                // Not logged in - redirect to login
                window.location.href = '/login.html?status=unauthorized';
                return;
            }

            if (meResp.status === 403) {
                // Pending approval
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

    // Admin panel
    async function openAdminPanel() {
        const modal = document.getElementById('admin-panel-modal');
        if (modal) {
            modal.classList.remove('hidden');
            await loadUsers();
        }
    }

    async function loadUsers() {
        const container = document.getElementById('admin-users-list');
        if (!container) return;

        try {
            const resp = await fetch('/api/admin/users');
            if (!resp.ok) {
                container.innerHTML = '<p>Failed to load users.</p>';
                return;
            }

            const users = await resp.json();
            renderUserList(container, users);
        } catch (e) {
            container.innerHTML = '<p>Error loading users.</p>';
            console.error('Failed to load users:', e);
        }
    }

    function renderUserList(container, users) {
        if (users.length === 0) {
            container.innerHTML = '<p>No users found.</p>';
            return;
        }

        const html = users.map(user => {
            const isPending = user.role === 'pending';
            const isCurrentUser = currentUser && user.id === currentUser.id;
            const roleClass = isPending ? 'role-pending' : (user.role === 'admin' ? 'role-admin' : 'role-user');
            const createdDate = new Date(user.created_at).toLocaleDateString();

            return `
                <div class="admin-user-card" data-user-id="${user.id}">
                    <div class="admin-user-info">
                        ${user.picture ? `<img class="admin-user-avatar" src="${user.picture}" alt="" referrerpolicy="no-referrer">` : '<div class="admin-user-avatar-placeholder">?</div>'}
                        <div>
                            <div class="admin-user-name">${escapeHtml(user.name)}${isCurrentUser ? ' (you)' : ''}</div>
                            <div class="admin-user-email">${escapeHtml(user.email)}</div>
                            <div class="admin-user-meta">Joined ${createdDate}</div>
                        </div>
                    </div>
                    <div class="admin-user-actions">
                        <span class="admin-user-role ${roleClass}">${user.role}</span>
                        ${!isCurrentUser ? `
                            <select class="admin-role-select" data-user-id="${user.id}" data-current-role="${user.role}">
                                <option value="pending" ${user.role === 'pending' ? 'selected' : ''}>Pending</option>
                                <option value="user" ${user.role === 'user' ? 'selected' : ''}>User</option>
                                <option value="admin" ${user.role === 'admin' ? 'selected' : ''}>Admin</option>
                            </select>
                            <button class="btn btn-secondary btn-sm admin-delete-user" data-user-id="${user.id}" title="Delete user">&#10005;</button>
                        ` : ''}
                    </div>
                </div>
            `;
        }).join('');

        container.innerHTML = html;

        // Attach event handlers
        container.querySelectorAll('.admin-role-select').forEach(select => {
            select.addEventListener('change', async (e) => {
                const userId = e.target.dataset.userId;
                const newRole = e.target.value;
                await updateUserRole(userId, newRole);
            });
        });

        container.querySelectorAll('.admin-delete-user').forEach(btn => {
            btn.addEventListener('click', async (e) => {
                const userId = e.target.dataset.userId || e.target.closest('[data-user-id]').dataset.userId;
                if (confirm('Are you sure you want to delete this user?')) {
                    await deleteUser(userId);
                }
            });
        });
    }

    async function updateUserRole(userId, role) {
        try {
            const resp = await fetch(`/api/admin/users/${userId}/role`, {
                method: 'PUT',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ role }),
            });

            if (!resp.ok) {
                const err = await resp.json();
                alert(`Failed to update role: ${err.message}`);
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

            if (!resp.ok) {
                const err = await resp.json();
                alert(`Failed to delete user: ${err.message}`);
            }

            await loadUsers();
        } catch (e) {
            console.error('Failed to delete user:', e);
            alert('Failed to delete user');
        }
    }

    function escapeHtml(str) {
        const div = document.createElement('div');
        div.textContent = str;
        return div.innerHTML;
    }

    // Handle modal close
    document.addEventListener('click', (e) => {
        if (e.target.matches('[data-close-modal="admin-panel-modal"]')) {
            const modal = document.getElementById('admin-panel-modal');
            if (modal) modal.classList.add('hidden');
        }
    });

    // Also intercept 401 responses globally to redirect to login
    const originalFetch = window.fetch;
    window.fetch = async function (...args) {
        const response = await originalFetch.apply(this, args);

        // Don't redirect for auth endpoints themselves
        const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
        if (url.includes('/api/auth/')) {
            return response;
        }

        if (response.status === 401) {
            // Check if auth is enabled before redirecting
            try {
                const statusResp = await originalFetch('/api/auth/status');
                const statusData = await statusResp.json();
                if (statusData.auth_enabled) {
                    window.location.href = '/login.html?status=expired';
                }
            } catch (e) {
                // Ignore - auth status check failed
            }
        }

        return response;
    };

    // Initialize on DOM ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initAuth);
    } else {
        initAuth();
    }
})();
