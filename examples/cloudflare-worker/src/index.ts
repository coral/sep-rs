import {
  modelProfiles,
  options,
  phoneOptions,
  validateArtifact,
  type OptionsTarget,
  type Protocol,
} from 'sep-tools/cloudflare'

const jsonHeaders = {
  'cache-control': 'no-store',
  'content-type': 'application/json; charset=utf-8',
}

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: jsonHeaders,
  })
}

function requiredParameter(url: URL, name: string): string {
  const value = url.searchParams.get(name)
  if (!value) {
    throw new Error(`missing query parameter: ${name}`)
  }
  return value
}

function protocolParameter(url: URL): Protocol {
  const protocol = requiredParameter(url, 'protocol')
  if (protocol !== 'sccp' && protocol !== 'sip') {
    throw new Error('protocol must be sccp or sip')
  }
  return protocol
}

async function handle(request: Request): Promise<Response> {
  const url = new URL(request.url)

  if (request.method === 'GET' && url.pathname === '/') {
    return json({
      endpoints: [
        'GET /models',
        'GET /options',
        'GET /options?target=device',
        'GET /phone-options?model=8841&protocol=sip',
        'POST /validate-artifact',
      ],
    })
  }

  if (request.method === 'GET' && url.pathname === '/models') {
    return json(modelProfiles())
  }

  if (request.method === 'GET' && url.pathname === '/options') {
    const target = url.searchParams.get('target') as OptionsTarget | null
    return json(options(target ?? undefined))
  }

  if (request.method === 'GET' && url.pathname === '/phone-options') {
    return json(
      phoneOptions(requiredParameter(url, 'model'), protocolParameter(url)),
    )
  }

  if (request.method === 'POST' && url.pathname === '/validate-artifact') {
    return json(validateArtifact(await request.json()))
  }

  return json({ error: 'not found' }, 404)
}

export default {
  async fetch(request: Request): Promise<Response> {
    try {
      return await handle(request)
    } catch (error) {
      return json(
        { error: error instanceof Error ? error.message : String(error) },
        400,
      )
    }
  },
}
