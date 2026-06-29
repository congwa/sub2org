import { resolve } from 'path';
import { mergeConfig, defineConfig } from 'vite';
import { crx, ManifestV3Export } from '@crxjs/vite-plugin';
import baseConfig, { baseManifest, baseBuildOptions } from './vite.config.base'

const outDir = resolve(__dirname, 'dist_chrome');

// M3 自托管自动更新（off-store）：仅在 release 时由 env 注入，避免把部署域名/公钥写进仓库。
// SUB2ORG_EXT_KEY = base64 公钥（钉死扩展 ID）；SUB2ORG_EXT_UPDATE_URL = updates.xml 的 https 地址。
// 由 scripts/pack-crx.mjs 生成并打印。Firefox 构建不读这两个值（Chrome 专属）。
const extKey = process.env.SUB2ORG_EXT_KEY;
const extUpdateUrl = process.env.SUB2ORG_EXT_UPDATE_URL;

export default mergeConfig(
  baseConfig,
  defineConfig({
    plugins: [
      crx({
        manifest: {
          ...baseManifest,
          ...(extKey ? { key: extKey } : {}),
          ...(extUpdateUrl ? { update_url: extUpdateUrl } : {}),
          background: {
            service_worker: 'src/pages/background/index.ts',
            type: 'module'
          },
        } as ManifestV3Export,
        browser: 'chrome',
        contentScripts: {
          injectCss: true,
        }
      })
    ],
    build: {
      ...baseBuildOptions,
      outDir
    },
  })
)