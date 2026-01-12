// @ts-check

(function () {
    // Get VS Code API
    const vscode = acquireVsCodeApi();

    // State
    let currentPersonality = 'kind';
    let currentModel = 'gemini-pro';
    let selectedFiles = [];
    let isLoading = false;

    // DOM Elements
    const chatArea = document.getElementById('chat-area');
    const userInput = document.getElementById('user-input');
    const sendBtn = document.getElementById('send-btn');
    const pickFilesBtn = document.getElementById('pick-files-btn');
    const selectedFilesContainer = document.getElementById('selected-files');
    const apiKeyWarning = document.getElementById('api-key-warning');
    const personalityButtons = document.querySelectorAll('.personality-btn');
    const modelSelect = document.getElementById('model-select');
    const fileCount = document.getElementById('file-count');

    // Initialize
    init();

    function init() {
        // Set up event listeners
        sendBtn.addEventListener('click', handleSend);
        userInput.addEventListener('keydown', handleKeyDown);
        userInput.addEventListener('input', autoResize);
        pickFilesBtn.addEventListener('click', handlePickFiles);
        modelSelect.addEventListener('change', handleModelChange);

        personalityButtons.forEach(btn => {
            btn.addEventListener('click', () => handlePersonalityChange(btn.dataset.personality));
        });

        // Check API key status
        vscode.postMessage({ type: 'checkApiKey' });

        // Focus input
        userInput.focus();
    }

    // Handle messages from extension
    window.addEventListener('message', event => {
        const message = event.data;

        switch (message.type) {
            case 'init':
                currentPersonality = message.personality;
                currentModel = message.model || 'gemini-pro';
                modelSelect.value = currentModel;
                updatePersonalityUI();
                if (message.history && message.history.length > 0) {
                    message.history.forEach(msg => {
                        if (msg.role === 'user') {
                            addUserMessage(msg.content);
                        } else {
                            addAssistantMessage(msg.content, msg.personality);
                        }
                    });
                }
                break;

            case 'userMessage':
                addUserMessage(message.message.content);
                break;

            case 'assistantMessage':
                addAssistantMessage(message.message.content, message.message.personality);
                break;

            case 'loading':
                setLoading(message.isLoading);
                break;

            case 'error':
                addErrorMessage(message.message);
                setLoading(false);
                break;

            case 'filesSelected':
                selectedFiles = message.files;
                updateSelectedFilesUI();
                break;

            case 'personalityChanged':
                currentPersonality = message.personality;
                updatePersonalityUI();
                break;

            case 'modelChanged':
                currentModel = message.model;
                modelSelect.value = currentModel;
                break;

            case 'apiKeyStatus':
                if (!message.hasKey) {
                    apiKeyWarning.style.display = 'block';
                } else {
                    apiKeyWarning.style.display = 'none';
                }
                break;

            case 'clearChat':
                chatArea.innerHTML = '';
                break;
        }
    });

    function handleSend() {
        const text = userInput.value.trim();
        if (!text || isLoading) {
            return;
        }

        vscode.postMessage({
            type: 'sendMessage',
            text: text,
            files: selectedFiles,
            personality: currentPersonality
        });

        userInput.value = '';
        userInput.style.height = 'auto';
    }

    function handleKeyDown(event) {
        if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault();
            handleSend();
        }
    }

    function handlePickFiles() {
        vscode.postMessage({ type: 'pickFiles' });
    }

    function handlePersonalityChange(personality) {
        currentPersonality = personality;
        vscode.postMessage({
            type: 'changePersonality',
            personality: personality
        });
        updatePersonalityUI();
    }

    function handleModelChange() {
        currentModel = modelSelect.value;
        vscode.postMessage({
            type: 'changeModel',
            model: currentModel
        });
    }

    function autoResize() {
        userInput.style.height = 'auto';
        userInput.style.height = Math.min(userInput.scrollHeight, 120) + 'px';
    }

    function updatePersonalityUI() {
        personalityButtons.forEach(btn => {
            if (btn.dataset.personality === currentPersonality) {
                btn.classList.add('active');
            } else {
                btn.classList.remove('active');
            }
        });
    }

    function updateSelectedFilesUI() {
        if (selectedFiles.length === 0) {
            selectedFilesContainer.innerHTML = '';
            fileCount.textContent = '';
            return;
        }

        fileCount.textContent = `(${selectedFiles.length})`;

        selectedFilesContainer.innerHTML = selectedFiles.map((file, index) => {
            const fileName = file.split('/').pop() || file.split('\\').pop() || file;
            return `
                <div class="file-tag">
                    <span>${fileName}</span>
                    <button class="file-tag-remove" data-index="${index}">×</button>
                </div>
            `;
        }).join('');

        // Add remove listeners
        document.querySelectorAll('.file-tag-remove').forEach(btn => {
            btn.addEventListener('click', () => {
                const index = parseInt(btn.dataset.index);
                selectedFiles.splice(index, 1);
                updateSelectedFilesUI();
            });
        });
    }

    function addUserMessage(content) {
        const messageDiv = document.createElement('div');
        messageDiv.className = 'message message-user';
        messageDiv.innerHTML = `
            <div class="message-header">You</div>
            <div class="message-content">${escapeHtml(content)}</div>
            <div class="message-timestamp">${getTimeString()}</div>
        `;
        chatArea.appendChild(messageDiv);
        scrollToBottom();
    }

    function addAssistantMessage(content, personality) {
        const messageDiv = document.createElement('div');
        messageDiv.className = 'message message-assistant';
        
        // Render markdown
        const renderedContent = renderMarkdown(content);
        
        const personalityLabel = {
            angry: 'Direct',
            kind: 'Supportive',
            wise: 'Architectural'
        }[personality] || 'Assistant';
        
        messageDiv.innerHTML = `
            <div class="message-header">${personalityLabel}</div>
            <div class="message-content">${renderedContent}</div>
            <div class="message-timestamp">${getTimeString()}</div>
        `;
        chatArea.appendChild(messageDiv);
        scrollToBottom();
    }

    function addErrorMessage(content) {
        const messageDiv = document.createElement('div');
        messageDiv.className = 'message message-assistant';
        messageDiv.innerHTML = `
            <div class="message-header">Error</div>
            <div class="message-content" style="color: var(--error-color);">${renderMarkdown(content)}</div>
            <div class="message-timestamp">${getTimeString()}</div>
        `;
        chatArea.appendChild(messageDiv);
        scrollToBottom();
    }

    function setLoading(loading) {
        isLoading = loading;
        sendBtn.disabled = loading;
        
        // Remove existing loading indicator
        const existingLoader = chatArea.querySelector('.loading-indicator');
        if (existingLoader) {
            existingLoader.remove();
        }

        if (loading) {
            const loaderDiv = document.createElement('div');
            loaderDiv.className = 'loading-indicator';
            loaderDiv.innerHTML = `
                <span>Processing...</span>
                <div class="loading-dots">
                    <div class="loading-dot"></div>
                    <div class="loading-dot"></div>
                    <div class="loading-dot"></div>
                </div>
            `;
            chatArea.appendChild(loaderDiv);
            scrollToBottom();
        }
    }

    function getPersonalityEmoji(personality) {
        const emojis = {
            angry: '😠',
            kind: '😊',
            wise: '🧙‍♀️'
        };
        return emojis[personality] || '🧙‍♀️';
    }

    function getTimeString() {
        const now = new Date();
        return now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }

    function scrollToBottom() {
        chatArea.scrollTop = chatArea.scrollHeight;
    }

    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    // Simple markdown renderer
    function renderMarkdown(text) {
        let html = escapeHtml(text);

        // Code blocks
        html = html.replace(/```(\w+)?\n([\s\S]*?)```/g, (match, lang, code) => {
            return `<pre><code>${code.trim()}</code></pre>`;
        });

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

        // Line breaks
        html = html.replace(/\n\n/g, '</p><p>');
        html = '<p>' + html + '</p>';

        // Clean up empty paragraphs
        html = html.replace(/<p><\/p>/g, '');
        html = html.replace(/<p>(<[uo]l>)/g, '$1');
        html = html.replace(/(<\/[uo]l>)<\/p>/g, '$1');
        html = html.replace(/<p>(<h[1-6]>)/g, '$1');
        html = html.replace(/(<\/h[1-6]>)<\/p>/g, '$1');
        html = html.replace(/<p>(<pre>)/g, '$1');
        html = html.replace(/(<\/pre>)<\/p>/g, '$1');

        return html;
    }
})();
