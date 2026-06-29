#!/usr/bin/env node
// 把 dist_chrome/ 打包签名成自托管 CRX（CRX3）+ 生成 updates.xml。
// 依赖：node、openssl、zip（macOS/Linux 自带）。无 npm 依赖。
//
// ⚠️ off-store 自动更新限制：消费版 Chrome 默认禁止非商店扩展的 update_url 自动更新，
//    只有经企业策略 force-install/allowlist 或在指纹/防关联浏览器上才会触发。详见 README / DEPLOY.md。
//
// 用法：
//   1) 首次：`node scripts/pack-crx.mjs`  → 生成 keys/sub2org-worker.pem，打印需要设置的 env。
//   2) 用打印的 env 重新构建：`SUB2ORG_EXT_KEY=… SUB2ORG_EXT_UPDATE_URL=…/ext/updates.xml yarn build:chrome`
//   3) 再打包：`SUB2ORG_EXT_UPDATE_BASE=https://<域名>/ext node scripts/pack-crx.mjs`
//      → dist_release/sub2org-worker-<version>.crx + dist_release/updates.xml
//
// 单次产出（release:chrome）只在 manifest 已含 key/update_url 时才是「可自动更新」的，
// 否则仍可手动安装，脚本会提示尚未烘焙。

import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const keysDir = path.join(root, "keys");
const pemPath = path.join(keysDir, "sub2org-worker.pem");
const distChrome = path.join(root, "dist_chrome");
const releaseDir = path.join(root, "dist_release");
const version = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")).version;
const updateBase = (process.env.SUB2ORG_EXT_UPDATE_BASE || "https://YOUR_DOMAIN/ext").replace(/\/+$/, "");

// ---- protobuf 小工具（CRX3 头部只用到 length-delimited 字段）----
const varint = (n) => {
  const out = [];
  while (n > 0x7f) {
    out.push((n & 0x7f) | 0x80);
    n = Math.floor(n / 128);
  }
  out.push(n);
  return Buffer.from(out);
};
const tag = (field) => varint((field << 3) | 2); // wire type 2
const lenField = (field, buf) => Buffer.concat([tag(field), varint(buf.length), buf]);
const u32le = (n) => {
  const b = Buffer.alloc(4);
  b.writeUInt32LE(n >>> 0);
  return b;
};

// ---- 1) 确保私钥存在 ----
fs.mkdirSync(keysDir, { recursive: true });
if (!fs.existsSync(pemPath)) {
  execFileSync("openssl", ["genrsa", "-out", pemPath, "2048"], { stdio: ["ignore", "ignore", "inherit"] });
  console.log(`[pack-crx] 已生成私钥 ${path.relative(root, pemPath)}（请妥善保管、勿入库）`);
}

// ---- 2) 推导 DER 公钥（= manifest key）与稳定扩展 ID ----
const derPub = execFileSync("openssl", ["rsa", "-in", pemPath, "-pubout", "-outform", "DER"], {
  stdio: ["ignore", "pipe", "ignore"],
});
const keyB64 = derPub.toString("base64");
const idHash = crypto.createHash("sha256").update(derPub).digest();
const extensionId = [...idHash.subarray(0, 16)]
  .map((b) => b.toString(16).padStart(2, "0"))
  .join("")
  .split("")
  .map((c) => String.fromCharCode(97 + parseInt(c, 16)))
  .join("");

console.log(`\n[pack-crx] 扩展 ID: ${extensionId}`);
console.log(`[pack-crx] 设置以下环境变量后重新构建，才能烘焙进 manifest：`);
console.log(`    export SUB2ORG_EXT_KEY='${keyB64}'`);
console.log(`    export SUB2ORG_EXT_UPDATE_URL='${updateBase}/updates.xml'\n`);

if (!fs.existsSync(distChrome)) {
  console.log("[pack-crx] 未发现 dist_chrome/，先 `yarn build:chrome` 再打包。仅生成了密钥信息。");
  process.exit(0);
}

// 检查 manifest 是否已烘焙 key/update_url
try {
  const m = JSON.parse(fs.readFileSync(path.join(distChrome, "manifest.json"), "utf8"));
  if (m.key !== keyB64 || !m.update_url) {
    console.log("[pack-crx] ⚠️ dist_chrome/manifest.json 尚未含正确的 key/update_url —— ");
    console.log("           本次产出可手动安装，但不会自动更新。请用上面的 env 重新 build:chrome 后再打包。\n");
  }
} catch {
  /* ignore */
}

// ---- 3) 打 zip ----
fs.mkdirSync(releaseDir, { recursive: true });
const zipPath = path.join(releaseDir, "dist_chrome.zip");
if (fs.existsSync(zipPath)) fs.rmSync(zipPath);
execFileSync("zip", ["-r", "-X", "-q", zipPath, "."], { cwd: distChrome });
const zipBuf = fs.readFileSync(zipPath);
fs.rmSync(zipPath);

// ---- 4) 组 CRX3 ----
const signedHeaderData = lenField(1, idHash.subarray(0, 16)); // SignedData{ crx_id }
const signMagic = Buffer.concat([Buffer.from("CRX3 SignedData", "latin1"), Buffer.from([0])]);
const toSign = Buffer.concat([signMagic, u32le(signedHeaderData.length), signedHeaderData, zipBuf]);
const signature = crypto.sign("RSA-SHA256", toSign, crypto.createPrivateKey(fs.readFileSync(pemPath)));

const proof = Buffer.concat([lenField(1, derPub), lenField(2, signature)]); // AsymmetricKeyProof
const header = Buffer.concat([lenField(2, proof), lenField(10000, signedHeaderData)]); // CrxFileHeader
const crx = Buffer.concat([Buffer.from("Cr24", "latin1"), u32le(3), u32le(header.length), header, zipBuf]);

const crxName = `sub2org-worker-${version}.crx`;
const crxPath = path.join(releaseDir, crxName);
fs.writeFileSync(crxPath, crx);

// ---- 5) updates.xml ----
const updatesXml = `<?xml version='1.0' encoding='UTF-8'?>
<gupdate xmlns='http://www.google.com/update2/response' protocol='2.0'>
  <app appid='${extensionId}'>
    <updatecheck codebase='${updateBase}/${crxName}' version='${version}' />
  </app>
</gupdate>
`;
fs.writeFileSync(path.join(releaseDir, "updates.xml"), updatesXml);

console.log(`[pack-crx] ✅ ${path.relative(root, crxPath)} (${(crx.length / 1024).toFixed(0)} KB)`);
console.log(`[pack-crx] ✅ ${path.relative(root, path.join(releaseDir, "updates.xml"))}`);
console.log(`[pack-crx] 把 dist_release/{${crxName},updates.xml} 部署到 ${updateBase}/`);
