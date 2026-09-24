import fs from 'node:fs/promises'
import path from 'node:path'

export interface ModulesManifestData {
  nodeLinker?: 'hoisted' | 'isolated' | 'pnp'
  shamefullyHoist?: boolean
  publicHoistPattern?: string[]
  hoistedLocations?: Record<string, string[]>
}

export async function readModulesManifest (modulesDir: string): Promise<ModulesManifestData | null> {
  const modulesYamlPath = path.join(modulesDir, '.modules.yaml')
  try {
    const rawManifest = await fs.readFile(modulesYamlPath, 'utf8')
    try {
      return JSON.parse(rawManifest) as ModulesManifestData
    } catch {
      const nodeLinkerMatch = rawManifest.match(/nodeLinker:\s*['"]?(\w+)['"]?/)
      const shamefullyHoistMatch = rawManifest.match(/shamefullyHoist:\s*(true|false)/)
      return {
        nodeLinker: nodeLinkerMatch ? (nodeLinkerMatch[1] as 'hoisted' | 'isolated' | 'pnp') : undefined,
        shamefullyHoist: shamefullyHoistMatch ? shamefullyHoistMatch[1] === 'true' : undefined,
      }
    }
  } catch (err: unknown) {
    if ((err as NodeJS.ErrnoException).code !== 'ENOENT') {
      throw err
    }
    return null
  }
}
