import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

(async () => {
    const currentWindow = getCurrentWindow();
    const canvas = document.getElementById('screenCanvas');
    const ctx = canvas.getContext('2d');
    const selectionRect = document.getElementById('selectionRect');
    const sizeIndicator = document.getElementById('sizeIndicator');
    const hint = document.getElementById('hint');

    let isSelecting = false;
    let startX = 0, startY = 0;
    let screenshotMode = 0; // 0=normal, 1=quick save, 2=quick pin, 3=quick OCR, 4=translate

    // Get mode from backend
    try {
        const data = await invoke('get_screenshot_select_data', { window: currentWindow });
        screenshotMode = data.mode || 0;
    } catch (e) {
        console.error('获取截图模式失败:', e);
    }

    // Load screenshot image onto canvas
    try {
        const imagePath = await invoke('get_screenshot_image_path');
        const assetUrl = convertFileSrc(imagePath.replace(/\\/g, '/'), 'asset');
        const img = new Image();
        img.onload = () => {
            // Use device pixel ratio for sharp rendering
            const dpr = window.devicePixelRatio || 1;
            canvas.width = window.innerWidth * dpr;
            canvas.height = window.innerHeight * dpr;
            canvas.style.width = window.innerWidth + 'px';
            canvas.style.height = window.innerHeight + 'px';
            ctx.scale(dpr, dpr);
            ctx.drawImage(img, 0, 0, window.innerWidth, window.innerHeight);
        };
        img.onerror = (e) => {
            console.error('加载截图图片失败:', e);
        };
        img.src = assetUrl;
    } catch (e) {
        console.error('获取截图路径失败:', e);
    }

    // Mouse events for selection
    document.addEventListener('mousedown', (e) => {
        if (e.button !== 0) return; // Left click only
        isSelecting = true;
        startX = e.clientX;
        startY = e.clientY;
        selectionRect.classList.add('active');
        selectionRect.style.left = `${startX}px`;
        selectionRect.style.top = `${startY}px`;
        selectionRect.style.width = '0px';
        selectionRect.style.height = '0px';
        hint.classList.add('hidden');
    });

    document.addEventListener('mousemove', (e) => {
        if (!isSelecting) return;

        const currentX = e.clientX;
        const currentY = e.clientY;

        const left = Math.min(startX, currentX);
        const top = Math.min(startY, currentY);
        const width = Math.abs(currentX - startX);
        const height = Math.abs(currentY - startY);

        selectionRect.style.left = `${left}px`;
        selectionRect.style.top = `${top}px`;
        selectionRect.style.width = `${width}px`;
        selectionRect.style.height = `${height}px`;

        // Size indicator in physical pixels
        const dpr = window.devicePixelRatio || 1;
        const physW = Math.round(width * dpr);
        const physH = Math.round(height * dpr);
        sizeIndicator.textContent = `${physW} × ${physH}`;
        sizeIndicator.classList.add('active');

        // Position indicator near the selection
        const indicatorLeft = left + width + 8;
        const indicatorTop = top + height + 8;
        // Keep indicator within viewport
        sizeIndicator.style.left = `${Math.min(indicatorLeft, window.innerWidth - 100)}px`;
        sizeIndicator.style.top = `${Math.min(indicatorTop, window.innerHeight - 30)}px`;
    });

    document.addEventListener('mouseup', async (e) => {
        if (!isSelecting) return;
        isSelecting = false;

        const currentX = e.clientX;
        const currentY = e.clientY;

        const left = Math.min(startX, currentX);
        const top = Math.min(startY, currentY);
        const width = Math.abs(currentX - startX);
        const height = Math.abs(currentY - startY);

        // Minimum selection size
        if (width < 5 || height < 5) {
            selectionRect.classList.remove('active');
            sizeIndicator.classList.remove('active');
            return;
        }

        const dpr = window.devicePixelRatio || 1;

        // Convert to physical pixels for the backend
        const physX = Math.round(left * dpr);
        const physY = Math.round(top * dpr);
        const physW = Math.round(width * dpr);
        const physH = Math.round(height * dpr);

        try {
            // Call backend to process the selection - backend will close this window
            await invoke('screenshot_selection_complete', {
                mode: screenshotMode,
                x: physX,
                y: physY,
                width: physW,
                height: physH
            });
        } catch (err) {
            console.error('截图完成处理失败:', err);
        }

        // Close the selection window as fallback
        try {
            await currentWindow.close();
        } catch (e) {
            // Window might already be closed by backend
        }
    });

    // ESC to cancel
    document.addEventListener('keydown', async (e) => {
        if (e.key === 'Escape') {
            try {
                await currentWindow.close();
            } catch (e) {
                // ignore
            }
        }
    });

    // Right click to cancel
    document.addEventListener('contextmenu', async (e) => {
        e.preventDefault();
        try {
            await currentWindow.close();
        } catch (e) {
            // ignore
        }
    });
})();
