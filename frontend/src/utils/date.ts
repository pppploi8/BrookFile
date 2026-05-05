const pad = (n: number): string => String(n).padStart(2, '0')

const formatParts = (d: Date): string => {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

export function formatUtcTimestamp(timestamp: number | string): string {
  return formatParts(new Date(parseInt(String(timestamp)) * 1000))
}

export function formatUtcDatetimeString(dateStr: string): string {
  let normalized = dateStr.includes('T') ? dateStr : dateStr.replace(' ', 'T')
  if (!normalized.endsWith('Z') && !/[+-]\d{2}:\d{2}$/.test(normalized)) {
    normalized += 'Z'
  }
  return formatParts(new Date(normalized))
}
