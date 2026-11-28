declare module '*.vue' {
  import type { DefineComponent } from 'vue'
  const component: DefineComponent<{}, {}, any>
  export default component
}

// Vite injects `import.meta.env.BASE_URL`, and the router reads it to learn the
// mount point (/admin/ live, / under `vite dev`). Without this declaration `tsc`
// treats it as an unknown property on `ImportMeta`.
interface ImportMetaEnv {
  readonly BASE_URL: string
}
interface ImportMeta {
  readonly env: ImportMetaEnv
}
