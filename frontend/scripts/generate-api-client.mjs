import { existsSync, mkdirSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const frontendRoot = resolve(__dirname, '..')
const repoRoot = resolve(frontendRoot, '..')
const backendRoot = join(repoRoot, 'backend')
const backendOpenApiTargetDir = join(backendRoot, 'target-openapi')
const localArvalezBin = join(
  frontendRoot,
  '.tools',
  'arvalez',
  'bin',
  process.platform === 'win32' ? 'arvalez-cli.exe' : 'arvalez-cli',
)
const defaultOpenApiCachePath = join(frontendRoot, '.generated', 'openapi', 'openapi.json')
const defaultOutputDirectory = join(frontendRoot, 'src', 'adapters', 'fumen-backend')

function printHelp() {
  console.log(`Generate the frontend TypeScript API client with Arvalez.

Usage:
  npm run api:generate
  npm run api:generate -- --openapi ./relative/path/to/openapi.json
  npm run api:generate -- --output ./src/adapters/fumen-backend

Options:
  --openapi <path>      Use an existing OpenAPI JSON file instead of running the backend export command.
  --output <path>       Output directory for the generated TypeScript client.
  --help                Show this help.
`)
}

function parseArgs(argv) {
  const options = {}

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]

    if (arg === '--help' || arg === '-h') {
      options.help = true
      continue
    }

    if (arg === '--openapi') {
      options.openapi = argv[index + 1]
      index += 1
      continue
    }

    if (arg === '--output') {
      options.output = argv[index + 1]
      index += 1
      continue
    }

    throw new Error(`Unknown argument: ${arg}`)
  }

  return options
}

function runCommand(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    stdio: 'inherit',
    shell: false,
  })

  if (result.status !== 0) {
    throw new Error(`Command failed: ${command} ${args.join(' ')}`)
  }
}

function ensureArvalezInstalled() {
  if (existsSync(localArvalezBin)) {
    return
  }

  console.log('Installing arvalez-cli into frontend/.tools/arvalez ...')
  runCommand(
    'cargo',
    ['install', '--git', 'https://github.com/Sygmei/arvalez', '--root', '.tools/arvalez', 'arvalez-cli'],
    frontendRoot,
  )
}

function exportOpenApiFromBackend(outputPath) {
  mkdirSync(dirname(outputPath), { recursive: true })
  console.log(`Exporting OpenAPI JSON with Cargo into ${outputPath}`)
  runCommand(
    'cargo',
    [
      'run',
      '--quiet',
      '--target-dir',
      backendOpenApiTargetDir,
      '--bin',
      'fumen-backend',
      '--',
      '--dump-openapi',
      outputPath,
    ],
    backendRoot,
  )
}

function resolveOpenApiPath(options) {
  if (options.openapi) {
    return resolve(frontendRoot, options.openapi)
  }

  exportOpenApiFromBackend(defaultOpenApiCachePath)
  return defaultOpenApiCachePath
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    printHelp()
    return
  }

  ensureArvalezInstalled()

  const openApiPath = resolveOpenApiPath(options)
  const outputDirectory = resolve(frontendRoot, options.output || defaultOutputDirectory)
  mkdirSync(outputDirectory, { recursive: true })

  console.log(`Generating TypeScript client from ${openApiPath}`)
  runCommand(
    localArvalezBin,
    ['generate-typescript', '--openapi', openApiPath, '--output-directory', outputDirectory],
    frontendRoot,
  )

  console.log(`TypeScript client generated in ${outputDirectory}`)
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
})
