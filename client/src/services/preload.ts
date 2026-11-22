import { getVergeConfig } from './cmds'
import {
  cacheLanguage,
  getCachedLanguage,
  initializeLanguage,
  resolveLanguage,
} from './i18n'

let vergeConfigCache: IVergeConfig | null | undefined

export const setPreloadConfig = (config: IVergeConfig | null) => {
  vergeConfigCache = config
}

export const getPreloadConfig = () => vergeConfigCache

const preloadConfig = async () => {
  try {
    const config = await getVergeConfig()
    setPreloadConfig(config)
    return config
  } catch (error) {
    console.warn('[preload.ts] Failed to read Verge config:', error)
    setPreloadConfig(null)
    return null
  }
}

const preloadLanguage = async (
  vergeConfig?: IVergeConfig | null,
  loadConfig: () => Promise<IVergeConfig | null> = preloadConfig,
) => {
  const cachedLanguage = getCachedLanguage()
  if (cachedLanguage) {
    return cachedLanguage
  }

  let resolvedConfig = vergeConfig

  if (resolvedConfig === undefined) {
    try {
      resolvedConfig = await loadConfig()
    } catch (error) {
      console.warn(
        '[preload.ts] Failed to read language from Verge config:',
        error,
      )
      resolvedConfig = null
    }
  }

  const languageFromConfig = resolvedConfig?.language
  if (languageFromConfig) {
    const resolved = resolveLanguage(languageFromConfig)
    cacheLanguage(resolved)
    return resolved
  }

  const browserLanguage = resolveLanguage(
    typeof navigator !== 'undefined' ? navigator.language : undefined,
  )
  cacheLanguage(browserLanguage)
  return browserLanguage
}

export const preloadAppData = async () => {
  const configPromise = preloadConfig()
  const initialLanguage = await preloadLanguage(undefined, () => configPromise)
  // The config is still preloaded: language lives in it. There is no theme mode
  // to resolve -- Chrysalis has one permanent appearance, so a stored `theme_mode` or
  // `theme_id` is ignored rather than read.
  await Promise.all([configPromise, initializeLanguage(initialLanguage)])
}
