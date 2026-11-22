import { createContextState } from 'foxact/create-context-state'

// save the state of each profile item loading
const [LoadingCacheProvider, useLoadingCache, useSetLoadingCache] =
  createContextState<Set<string>>(new Set())

// There is no theme-mode context: Locus has one permanent appearance
// (`services/theme.ts`), so there is no mode for a provider to carry. Removing
// it also removes a context whose value could change identity on every render
// and defeat every `memo` below it.
export { LoadingCacheProvider, useLoadingCache, useSetLoadingCache }
