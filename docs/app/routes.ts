import { index, type RouteConfig, route } from "@react-router/dev/routes";

export default [
  index("routes/home.tsx"),
  route("/llms-full.txt", "routes/llms-full.txt.ts"),
  route("/llms.txt", "routes/llms.txt.tsx"),
  route("playground", "routes/playground.tsx"),
  route("showcase", "routes/showcase.tsx"),
  route("docs/*", "docs/page.tsx"),
  route("/og/docs/*", "docs/og.tsx"),
  route("api/search", "docs/search.ts"),
  route("/sitemap.xml", "routes/sitemap.ts"),
] satisfies RouteConfig;
