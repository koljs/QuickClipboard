import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

(async () => {
    const currentWindow = getCurrentWindow();
    const img = document.getElementById('screenshotImage');
    const translationLayer = document.getElementById('translationLayer');
    const loadingOverlay = document.getElementById('loadingOverlay');

    const btnOriginal = document.getElementById('btnOriginal');
    const btnTranslation = document.getElementById('btnTranslation');
    const btnBilingual = document.getElementById('btnBilingual');
    const btnCopy = document.getElementById('btnCopy');
    const btnClose = document.getElementById('btnClose');

    let currentMode = 'translation';
    let overlayData = null;

    // Get data from backend
    try {
        overlayData = await invoke('get_translate_overlay_data', { window: currentWindow });
    } catch (error) {
        console.error('获取翻译覆盖层数据失败:', error);
        return;
    }

    if (!overlayData) return;

    // Load screenshot image
    const assetUrl = convertFileSrc(overlayData.image_path, 'asset');
    img.src = assetUrl;
    await new Promise((resolve, reject) => {
        img.onload = resolve;
        img.onerror = reject;
    });

    // Set window size based on image
    const dpr = window.devicePixelRatio || 1;
    const logicalWidth = img.naturalWidth / dpr;
    const logicalHeight = img.naturalHeight / dpr;
    img.style.width = `${logicalWidth}px`;
    img.style.height = `${logicalHeight}px`;

    const { LogicalSize, PhysicalPosition } = await import('@tauri-apps/api/window');
    const textScale = await invoke('get_system_text_scale');
    const padding = 5;
    await currentWindow.setSize(new LogicalSize(
        (logicalWidth + padding * 2) * textScale,
        (logicalHeight + padding * 2 + 36) * textScale
    ));

    if (overlayData.physical_x != null && overlayData.physical_y != null) {
        await currentWindow.setPosition(new PhysicalPosition(
            overlayData.physical_x - padding,
            overlayData.physical_y - padding
        ));
    }

    // If translations already available, render them
    if (overlayData.translations && overlayData.translations.length > 0) {
        renderTranslations(overlayData.ocr_lines, overlayData.translations, logicalWidth, logicalHeight);
        loadingOverlay.style.display = 'none';
    } else if (overlayData.ocr_lines) {
        // Need to translate - show loading and request translation
        renderTranslations(overlayData.ocr_lines, null, logicalWidth, logicalHeight);

        try {
            const result = await invoke('translate_ocr_lines_cmd', {
                lines: overlayData.ocr_lines.map(l => l.text),
                targetLanguage: overlayData.target_language || 'zh-CN'
            });

            overlayData.translations = result;
            renderTranslations(overlayData.ocr_lines, result, logicalWidth, logicalHeight);
            loadingOverlay.style.display = 'none';
        } catch (error) {
            console.error('翻译失败:', error);
            loadingOverlay.innerHTML = '<div class="loading-text" style="color: #ff6b6b;">翻译失败: ' + error + '</div>';
        }
    }

    function renderTranslations(ocrLines, translations, imgWidth, imgHeight) {
        translationLayer.innerHTML = '';
        translationLayer.style.width = `${imgWidth}px`;
        translationLayer.style.height = `${imgHeight}px`;

        ocrLines.forEach((line, index) => {
            const block = document.createElement('div');
            block.className = 'translation-block';

            // Position as percentage of image dimensions
            const left = (line.x / imgWidth) * 100;
            const top = (line.y / imgHeight) * 100;
            const width = (line.width / imgWidth) * 100;
            const height = (line.height / imgHeight) * 100;

            block.style.left = `${left}%`;
            block.style.top = `${top}%`;
            block.style.width = `${width}%`;
            block.style.height = `${height}%`;

            // Calculate font size based on line height
            const lineHeightPx = (line.height / imgHeight) * imgHeight;
            const fontSize = Math.max(10, Math.min(lineHeightPx * 0.75, 24));
            block.style.fontSize = `${fontSize}px`;
            block.style.lineHeight = `${lineHeightPx}px`;

            if (translations && translations[index]) {
                const transText = document.createElement('span');
                transText.className = 'translation-text';
                transText.textContent = translations[index];
                block.appendChild(transText);
            }

            // Original text (for bilingual mode)
            const origText = document.createElement('span');
            origText.className = 'original-text';
            origText.textContent = line.text;
            block.appendChild(origText);

            translationLayer.appendChild(block);
        });

        updateMode(currentMode);
    }

    function updateMode(mode) {
        currentMode = mode;
        document.body.className = `mode-${mode}`;

        [btnOriginal, btnTranslation, btnBilingual].forEach(btn => btn.classList.remove('active'));
        if (mode === 'original') btnOriginal.classList.add('active');
        else if (mode === 'translation') btnTranslation.classList.add('active');
        else if (mode === 'bilingual') btnBilingual.classList.add('active');
    }

    // Button handlers
    btnOriginal.addEventListener('click', () => updateMode('original'));
    btnTranslation.addEventListener('click', () => updateMode('translation'));
    btnBilingual.addEventListener('click', () => updateMode('bilingual'));

    btnCopy.addEventListener('click', async () => {
        if (overlayData.translations) {
            try {
                await invoke('copy_text_to_clipboard', { text: overlayData.translations.join('\n') });
                btnCopy.textContent = '✓';
                setTimeout(() => { btnCopy.textContent = '复制'; }, 1500);
            } catch (e) {
                console.error('复制失败:', e);
            }
        }
    });

    btnClose.addEventListener('click', async () => {
        await currentWindow.close();
    });

    // Drag to move window
    let isDragging = false;
    let dragStartX, dragStartY;

    img.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return;
        isDragging = true;
        dragStartX = e.screenX;
        dragStartY = e.screenY;
        e.preventDefault();
    });

    document.addEventListener('mousemove', (e) => {
        if (!isDragging) return;
        const dx = e.screenX - dragStartX;
        const dy = e.screenY - dragStartY;
        currentWindow.startDragging();
        isDragging = false;
    });

    document.addEventListener('mouseup', () => {
        isDragging = false;
    });

    // Double click to close
    img.addEventListener('dblclick', async () => {
        await currentWindow.close();
    });

    // Escape to close
    document.addEventListener('keydown', async (e) => {
        if (e.key === 'Escape') {
            await currentWindow.close();
        }
    });

    // Set initial mode
    updateMode('translation');
})();
