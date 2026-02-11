// @ts-check
/* ═══════════════════════════════════════════════════════════
   Debuggatha — Webview script
   Action-based UI with personality selector and multi-model support
   ═══════════════════════════════════════════════════════════ */

(function () {
    const vscode = acquireVsCodeApi();

    // ── State ────────────────────────────────────────────────
    let currentPersonality = 'kind';
    let currentProvider = '';
    let currentModel = '';
    let providers = [];
    let selectedFiles = [];
    let isLoading = false;

    // ── DOM refs ─────────────────────────────────────────────
    const $ = (sel) => document.querySelector(sel);
    const $$ = (sel) => document.querySelectorAll(sel);

    const outputArea       = $('#output-area');
    const userInput        = $('#user-input');
    const sendBtn          = $('#send-btn');
    const pickFilesBtn     = $('#pick-files-btn');
    const fileCountBadge   = $('#file-count');
    const selectedFilesEl  = $('#selected-files');
    const providerSelect   = $('#provider-select');
    const modelSelect      = $('#model-select');
    const apiKeyBanner     = $('#api-key-banner');
    const apiKeyMsg        = $('#api-key-msg');
    const configureKeyBtn  = $('#configure-key-btn');
    const personalityBtns  = $$('.personality-btn');
    const actionBtns       = $$('.action-btn');

    // ── Init ─────────────────────────────────────────────────
    init();

    function init() {
        sendBtn.addEventListener('click', handleSend);
        userInput.addEventListener('keydown', handleKeyDown);
        userInput.addEventListener('input', autoResize);
        pickFilesBtn.addEventListener('click', () => vscode.postMessage({ type: 'pickFiles' }));
        configureKeyBtn.addEventListener('click', () => vscode.postMessage({ type: 'configureApiKey' }));

        providerSelect.addEventListener('change', () => {
            vscode.postMessage({ type: 'changeProvider', provider: providerSelect.value });
        });
        modelSelect.addEventListener('change', () => {
            vscode.postMessage({ type: 'changeModel', model: modelSelect.value });
        });

        personalityBtns.forEach(btn => {
            btn.addEventListener('click', () => {
                currentPersonality = btn.dataset.personality;
                vscode.postMessage({ type: 'changePersonality', personality: currentPersonality });
                updatePersonalityUI();
            });
        });

        actionBtns.forEach(btn => {
            btn.addEventListener('click', () => {
                if (isLoading) { return; }
                vscode.postMessage({
                    type: 'executeAction',
                    action: btn.dataset.action,
                    personality: currentPersonality,
                });
            });
        });

        // Tell extension we're ready
        vscode.postMessage({ type: 'ready' });
    }

    // ── Incoming messages ────────────────────────────────────
    window.addEventListener('message', (event) => {
        const msg = event.data;
        switch (msg.type) {
            case 'init':
                currentPersonality = msg.personality;
                currentProvider = msg.provider;
                currentModel = msg.model;
                providers = msg.providers || [];
                selectedFiles = msg.files || [];
                buildProviderDropdown();
                buildModelDropdown();
                updatePersonalityUI();
                updateFilesUI();
                break;

            case 'assistantMessage':
                addOutputBlock(msg.message);
                break;

            case 'loading':
                setLoading(msg.isLoading, msg.label);
                break;

            case 'error':
                addErrorBlock(msg.message);
                setLoading(false);
                break;

            case 'filesSelected':
                selectedFiles = msg.files || [];
                updateFilesUI();
                break;

            case 'fileRemoved':
                selectedFiles = msg.files || [];
                updateFilesUI();
                break;

            case 'personalityChanged':
                currentPersonality = msg.personality;
                updatePersonalityUI();
                break;

            case 'providerChanged':
                currentProvider = msg.provider;
                providerSelect.value = currentProvider;
                // Rebuild model dropdown with new provider's models
                if (msg.models) {
                    const prov = providers.find(p => p.id === currentProvider);
                    if (prov) { prov.models = msg.models; }
                }
                buildModelDropdown();
                break;

            case 'modelChanged':
                currentModel = msg.model;
                modelSelect.value = currentModel;
                break;

            case 'apiKeyStatus':
                if (!msg.hasKey) {
                    apiKeyBanner.style.display = 'flex';
                    apiKeyMsg.textContent = `No API key for ${providerLabel(msg.provider)}.`;
                } else {
                    apiKeyBanner.style.display = 'none';
                }
                break;

            case 'clearOutput':
                outputArea.innerHTML = '<div class="output-empty"><p>Select files and run an action to begin.</p></div>';
                break;
        }
    });

    // ── Send follow-up ───────────────────────────────────────
    function handleSend() {
        const text = userInput.value.trim();
        if (!text || isLoading) { return; }
        vscode.postMessage({
            type: 'sendFollowUp',
            text,
            personality: currentPersonality,
        });
        userInput.value = '';
        userInput.style.height = 'auto';
    }

    function handleKeyDown(e) {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    }

    function autoResize() {
        userInput.style.height = 'auto';
        userInput.style.height = Math.min(userInput.scrollHeight, 120) + 'px';
    }

    // ── Dropdowns ────────────────────────────────────────────
    function buildProviderDropdown() {
        providerSelect.innerHTML = '';
        providers.forEach(p => {
            const opt = document.createElement('option');
            opt.value = p.id;
            opt.textContent = p.label;
            if (p.id === currentProvider) { opt.selected = true; }
            providerSelect.appendChild(opt);
        });
    }

    function buildModelDropdown() {
        modelSelect.innerHTML = '';
        const prov = providers.find(p => p.id === currentProvider);
        if (!prov) { return; }
        prov.models.forEach(m => {
            const opt = document.createElement('option');
            opt.value = m.id;
            opt.textContent = m.label;
            if (m.id === currentModel) { opt.selected = true; }
            modelSelect.appendChild(opt);
        });
        // If current model isn't in the list, select the first one
        if (prov.models.length > 0 && !prov.models.find(m => m.id === currentModel)) {
            currentModel = prov.models[0].id;
            modelSelect.value = currentModel;
        }
    }

    function providerLabel(id) {
        const p = providers.find(pr => pr.id === id);
        return p ? p.label : id;
    }

    // ── Personality UI ───────────────────────────────────────
    function updatePersonalityUI() {
        personalityBtns.forEach(btn => {
            btn.classList.toggle('active', btn.dataset.personality === currentPersonality);
        });
    }

    // ── File UI ──────────────────────────────────────────────
    function updateFilesUI() {
        fileCountBadge.textContent = selectedFiles.length > 0 ? String(selectedFiles.length) : '';

        if (selectedFiles.length === 0) {
            selectedFilesEl.innerHTML = '';
            return;
        }

        selectedFilesEl.innerHTML = selectedFiles
            .map((f, i) => {
                const name = f.split('/').pop() || f;
                return `<div class="file-tag"><span title="${esc(f)}">${esc(name)}</span><button class="file-tag-remove" data-index="${i}">&times;</button></div>`;
            })
            .join('');

        selectedFilesEl.querySelectorAll('.file-tag-remove').forEach(btn => {
            btn.addEventListener('click', () => {
                vscode.postMessage({ type: 'removeFile', index: parseInt(btn.dataset.index, 10) });
            });
        });
    }

    // ── Output rendering ─────────────────────────────────────
    function clearEmpty() {
        const empty = outputArea.querySelector('.output-empty');
        if (empty) { empty.remove(); }
    }

    function addOutputBlock(message) {
        clearEmpty();

        const actionLabels = { report: 'Report', audit: 'Audit', analysis: 'Analysis' };
        const personalityLabels = { kind: 'Kind Witch', wise: 'Technical Witch', angry: 'Mean Witch' };

        const block = document.createElement('div');
        block.className = 'output-block';

        const headerParts = [];
        if (message.action) { headerParts.push(`<span class="action-label">${actionLabels[message.action] || ''}</span>`); }
        if (message.personality) { headerParts.push(personalityLabels[message.personality] || ''); }
        headerParts.push(timeStr());

        block.innerHTML = `
            <div class="output-header">${headerParts.join(' · ')}</div>
            <div class="output-content">${renderMarkdown(message.content)}</div>
        `;
        outputArea.appendChild(block);
        scrollToBottom();
    }

    function addErrorBlock(text) {
        clearEmpty();
        const block = document.createElement('div');
        block.className = 'output-block output-error';
        block.innerHTML = `
            <div class="output-header">Error · ${timeStr()}</div>
            <div class="output-content">${renderMarkdown(text)}</div>
        `;
        outputArea.appendChild(block);
        scrollToBottom();
    }

    // ── Loading ──────────────────────────────────────────────
    function setLoading(loading, label) {
        isLoading = loading;
        sendBtn.disabled = loading;
        actionBtns.forEach(b => (b.disabled = loading));

        const existing = outputArea.querySelector('.loading-indicator');
        if (existing) { existing.remove(); }

        if (loading) {
            clearEmpty();
            const el = document.createElement('div');
            el.className = 'loading-indicator';
            el.innerHTML = `<span>${esc(label || 'Processing…')}</span>
                <div class="loading-dots"><div class="loading-dot"></div><div class="loading-dot"></div><div class="loading-dot"></div></div>`;
            outputArea.appendChild(el);
            scrollToBottom();
        }
    }

    // ── Utilities ────────────────────────────────────────────
    function timeStr() {
        return new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }

    function scrollToBottom() {
        outputArea.scrollTop = outputArea.scrollHeight;
    }

    function esc(text) {
        const d = document.createElement('div');
        d.textContent = text;
        return d.innerHTML;
    }

    // ── Markdown renderer ────────────────────────────────────
    function renderMarkdown(text) {
        let html = esc(text);

        // Fenced code blocks
        html = html.replace(/```(\w+)?\n([\s\S]*?)```/g, (_, lang, code) =>
            `<pre><code>${code.trim()}</code></pre>`);

        // Inline code
        html = html.replace(/`([^`]+)`/g, '<code>$1</code>');

        // Bold
        html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');

        // Headers
        html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
        html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
        html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>');

        // Lists
        html = html.replace(/^\* (.+)$/gm, '<li>$1</li>');
        html = html.replace(/^- (.+)$/gm, '<li>$1</li>');
        html = html.replace(/(<li>.*<\/li>\n?)+/g, '<ul>$&</ul>');

        // Paragraphs
        html = html.replace(/\n\n/g, '</p><p>');
        html = '<p>' + html + '</p>';

        // Clean up
        html = html.replace(/<p><\/p>/g, '');
        html = html.replace(/<p>(<[uoh])/g, '$1');
        html = html.replace(/(<\/[uoh][l1-6]?>)<\/p>/g, '$1');
        html = html.replace(/<p>(<pre>)/g, '$1');
        html = html.replace(/(<\/pre>)<\/p>/g, '$1');

        return html;
    }
})();
