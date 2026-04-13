export type StemQualityProfile = 'standard' | 'small' | 'very-small' | 'tiny'

export const STEM_QUALITY_PROFILES: Array<{
  value: StemQualityProfile
  label: string
  description: string
}> = [
  {
    value: 'standard',
    label: 'Standard',
    description: 'Keep the MuseScore-rendered OGG stems as-is with no extra compression.',
  },
  {
    value: 'small',
    label: 'Small',
    description: 'Apply light Opus recompression for smaller stem files.',
  },
  {
    value: 'very-small',
    label: 'Very small',
    description: 'Apply stronger Opus recompression to reduce stem size further.',
  },
  {
    value: 'tiny',
    label: 'Tiny',
    description: 'Apply the strongest Opus recompression for the smallest stored stems.',
  },
]
