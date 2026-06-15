#!/usr/bin/env node
// 社区版编译脚本 - 使用 --no-default-features 排除私有插件
import { spawn } from 'child_process';
import { fileURLToPath } from 'url';
import path from 'path';
import fs from 'fs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(__dirname, '..');

const isDev = process.argv.includes('--dev');
const isCommunity = process.argv.includes('--no-default-features') || !process.argv.includes('--full');
const command = isDev ? 'dev' : 'build';

const screenshotCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'screenshot.json');
const defaultCapabilityPath = path.join(rootDir, 'src-tauri', 'capabilities', 'default.json');
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
const cargoLockPath = path.join(rootDir, 'src-tauri', 'Cargo.lock');

function patchCapabilityFile(filePath) {
    if (!fs.existsSync(filePath)) return () => {};

    const original = fs.readFileSync(filePath, 'utf8');
    let json;
    try {
        json = JSON.parse(original);
    } catch {
        return () => {};
    }

    if (!Array.isArray(json.permissions)) return () => {};

    const nextPermissions = json.permissions.filter((p) => p !== 'screenshot-suite:default');
    if (nextPermissions.length === json.permissions.length) return () => {};

    json.permissions = nextPermissions;
    fs.writeFileSync(filePath, `${JSON.stringify(json, null, 2)}\n`, 'utf8');

    return () => {
        fs.writeFileSync(filePath, original, 'utf8');
    };
}

function patchCapabilitiesForCommunity() {
    if (!isCommunity) return () => {};

    const restoreScreenshot = patchCapabilityFile(screenshotCapabilityPath);
    const restoreDefault = patchCapabilityFile(defaultCapabilityPath);

    return () => {
        restoreScreenshot();
        restoreDefault();
    };
}

// 社区版需要临时移除 SSH git 依赖，否则 Cargo 会尝试拉取私有仓库
function patchCargoTomlForCommunity() {
    if (!isCommunity) return () => {};

    const original = fs.readFileSync(cargoTomlPath, 'utf8');
    let patched = original;

    // 注释掉 gpu-image-viewer 的 SSH git 依赖行
    patched = patched.replace(
        /^(gpu-image-viewer\s*=\s*\{[^}]*ssh:\/\/[^\n]*)$/m,
        '# $1'
    );
    // 也注释掉本地路径替代行（如果取消注释的话）
    patched = patched.replace(
        /^(#\s*gpu-image-viewer\s*=\s*\{\s*path\s*=)/m,
        '# $1'
    );
    // 注释掉 screenshot-suite 本地路径依赖（社区版 submodule 不存在）
    patched = patched.replace(
        /^(screenshot-suite\s*=\s*\{\s*path\s*=\s*"plugins\/screenshot-suite"[^}]*)\}/m,
        '# $1 }'
    );
    // 注释掉 gpu-image-viewer feature 定义
    patched = patched.replace(
        /^(gpu-image-viewer\s*=\s*\["dep:gpu-image-viewer"\])$/m,
        '# $1'
    );
    // 注释掉 screenshot-suite feature 定义
    patched = patched.replace(
        /^(screenshot-suite\s*=\s*\["dep:screenshot-suite"\])$/m,
        '# $1'
    );
    // 从 default features 中移除 gpu-image-viewer 和 screenshot-suite
    patched = patched.replace(
        /^(default\s*=\s*\[)([^\]]*)\]$/m,
        (match, prefix, features) => {
            const items = features.split(',').map(s => s.trim()).filter(s => s && s !== '"gpu-image-viewer"' && s !== '"screenshot-suite"');
            return `${prefix}${items.join(', ')}]`;
        }
    );

    if (patched !== original) {
        fs.writeFileSync(cargoTomlPath, patched, 'utf8');
        console.log('[build] 已临时移除 gpu-image-viewer SSH 依赖和 feature');

        // 删除 Cargo.lock 以强制重新解析依赖
        if (fs.existsSync(cargoLockPath)) {
            const lockOriginal = fs.readFileSync(cargoLockPath, 'utf8');
            fs.unlinkSync(cargoLockPath);
            console.log('[build] 已删除 Cargo.lock 以重新解析依赖');

            return () => {
                fs.writeFileSync(cargoTomlPath, original, 'utf8');
                fs.writeFileSync(cargoLockPath, lockOriginal, 'utf8');
                console.log('[build] 已恢复 Cargo.toml 和 Cargo.lock');
            };
        }

        return () => {
            fs.writeFileSync(cargoTomlPath, original, 'utf8');
            console.log('[build] 已恢复 Cargo.toml');
        };
    }

    return () => {};
}

const args = ['run', 'tauri', '--', command];
if (isCommunity) {
    args.push('--', '--no-default-features');
}

const edition = isCommunity ? '社区版' : '完整版';
console.log(`[build] 版本: ${edition}`);
console.log(`[build] 模式: ${isDev ? '开发' : '生产'}`);
console.log(`[build] 执行: npm ${args.join(' ')}`);

let restored = false;
const restoreCapabilities = patchCapabilitiesForCommunity();
const restoreCargoToml = patchCargoTomlForCommunity();
const restoreOnce = () => {
    if (restored) return;
    restored = true;
    try {
        restoreCargoToml();
    } catch {}
    try {
        restoreCapabilities();
    } catch {}
};

process.on('SIGINT', () => {
    restoreOnce();
    process.exit(130);
});

process.on('SIGTERM', () => {
    restoreOnce();
    process.exit(143);
});

const child = spawn('npm', args, { 
    stdio: 'inherit', 
    cwd: rootDir,
    shell: true,
    env: {
        ...process.env,
        QC_COMMUNITY: isCommunity ? '1' : '0'
    }
});

child.on('error', (err) => {
    restoreOnce();
    console.error(`[build] 启动失败: ${err.message}`);
    process.exit(1);
});

child.on('close', (code) => {
    restoreOnce();
    if (code !== 0) {
        console.error(`[build] 编译失败，退出码: ${code}`);
    } else {
        console.log(`[build] ${edition}编译完成`);
    }
    process.exit(code);
});
