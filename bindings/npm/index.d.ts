export type Protocol = 'sccp' | 'sip'
export type SignalingMode = 'non_secure' | 'authenticated' | 'encrypted'
export type Transport = 'udp' | 'tcp' | 'tls'

export type ArtifactDialect =
  | 'enterprise_xml'
  | 'legacy_sip_text'
  | 'compiled_binary'
  | 'signed_xml'
  | 'encrypted_xml'
  | 'mpp3pcc'

export type ArtifactKind =
  | 'device_configuration'
  | 'default_configuration'
  | 'load_descriptor'
  | 'firmware'
  | 'dial_plan'
  | 'soft_key_policy'
  | 'locale'
  | 'trust_list'
  | 'other'

export interface Diagnostic {
  severity: 'error' | 'warning'
  code: string
  path?: string
  message: string
}

export interface ValidationResult {
  detection?: {
    dialect: ArtifactDialect
    kind: ArtifactKind
  }
  valid: boolean
  diagnostics: Diagnostic[]
}

export interface ModelProfile {
  id: string
  display_name: string
  aliases: string[]
  model_id: number
  protocols: Protocol[]
  dialects: ArtifactDialect[]
  load_prefixes: string[]
}

export interface BundleFile {
  filename: string
  contents?: string
}

export interface CallControlEndpoint {
  host: string
  port: number
  priority?: number
  transport?: Transport
}

export interface SccpSpec {
  signaling?: SignalingMode
  keepalive_seconds?: number
}

export interface SipLine {
  index: number
  directory_number: string
  display_name?: string
  auth_name?: string
  auth_secret?: string
}

export type SipButton = { position: number } & (
  | { feature: 'line'; line_index: number }
  | { feature: 'speed_dial'; label: string; target: string }
  | { feature: 'service_uri'; label: string; uri: string }
  | { feature: 'blf'; label: string; target: string }
  | { feature: 'intercom'; line_index: number }
  | { feature: 'raw'; feature_id: number; label?: string; target?: string }
)

export interface SipSpec {
  signaling?: SignalingMode
  lines?: SipLine[]
  buttons?: SipButton[]
  timers?: {
    register_expires_seconds?: number
    invite_expires_seconds?: number
    keepalive_seconds?: number
  }
  media_ports?: { start: number; end: number }
  outbound_proxy?: string
}

export interface ServiceUrls {
  services?: string
  directory?: string
  messages?: string
  information?: string
  idle?: string
}

export interface DeviceSpec {
  mac: string
  model: string
  firmware?: string
  protocol: { sccp: SccpSpec } | { sip: SipSpec }
  endpoints: CallControlEndpoint[]
  phone_label?: string
  time_zone?: string
  date_template?: string
  ntp_server?: string
  locale?: string
  services?: ServiceUrls
}

export interface DefaultSpec {
  protocol: Protocol
  firmware?: string
  endpoints?: CallControlEndpoint[]
  model_loads?: Array<{ model: string; model_id?: number; firmware: string }>
  time_zone?: string
  date_template?: string
  ntp_server?: string
  locale?: string
}

export interface ExternalArtifact {
  filename: string
  kind: ArtifactKind
  required?: boolean
  description?: string
}

export interface BundleSpec {
  devices?: DeviceSpec[]
  defaults?: DefaultSpec[]
  external_artifacts?: ExternalArtifact[]
}

export interface GeneratedArtifact {
  filename: string
  kind: ArtifactKind
  dialect: ArtifactDialect
  contents: string
  contains_secrets: boolean
  warnings: Diagnostic[]
}

export interface BootstrapBundle {
  artifacts: GeneratedArtifact[]
  inventory: {
    files: Array<{
      filename: string
      kind: ArtifactKind
      source: 'generated' | 'external'
      required: boolean
      description?: string
    }>
  }
}

export function modelProfiles(): ModelProfile[]
export function validateArtifact(request: {
  filename: string
  contents: string
  model?: string
}): ValidationResult
export function validateBundle(request: { files: BundleFile[] }): ValidationResult
export function generateDevice(request: DeviceSpec): GeneratedArtifact
export function generateBundle(request: BundleSpec): BootstrapBundle
