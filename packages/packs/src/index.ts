import type { ReviewPack } from "@debuggatha/core";
import { accessibilityPack } from "./packs/accessibility.js";
import { architecturePack } from "./packs/architecture.js";
import { aspnetcorePack } from "./packs/aspnetcore.js";
import { astroPack } from "./packs/astro.js";
import { bunPack } from "./packs/bun.js";
import { dartPack } from "./packs/dart.js";
import { dxPack } from "./packs/dx.js";
import { electronPack } from "./packs/electron.js";
import { elysiaPack } from "./packs/elysia.js";
import { expressPack } from "./packs/express.js";
import { fastifyPack } from "./packs/fastify.js";
import { flutterPack } from "./packs/flutter.js";
import { goPack } from "./packs/go.js";
import { honoPack } from "./packs/hono.js";
import { javascriptPack } from "./packs/javascript.js";
import { kotlinPack } from "./packs/kotlin.js";
import { laravelPack } from "./packs/laravel.js";
import { litPack } from "./packs/lit.js";
import { nodejsPack } from "./packs/nodejs.js";
import { owaspPack } from "./packs/owasp.js";
import { performancePack } from "./packs/performance.js";
import { phpPack } from "./packs/php.js";
import { reactPack } from "./packs/react.js";
import { releasePack } from "./packs/release.js";
import { rustPack } from "./packs/rust.js";
import { sveltePack } from "./packs/svelte.js";
import { tauriPack } from "./packs/tauri.js";
import { typescriptPack } from "./packs/typescript.js";
import { vuePack } from "./packs/vue.js";

export const reviewPacks: ReviewPack[] = [
  typescriptPack,
  javascriptPack,
  rustPack,
  goPack,
  kotlinPack,
  dartPack,
  phpPack,
  bunPack,
  nodejsPack,
  reactPack,
  vuePack,
  sveltePack,
  astroPack,
  litPack,
  expressPack,
  fastifyPack,
  honoPack,
  elysiaPack,
  laravelPack,
  aspnetcorePack,
  tauriPack,
  electronPack,
  flutterPack,
  owaspPack,
  accessibilityPack,
  performancePack,
  architecturePack,
  dxPack,
  releasePack,
];
