export function buildExportFileName(
  deviceLabel: string,
  mode: 'all' | 'filtered',
  now: Date = new Date(),
): string {
  const safe =
    deviceLabel.replace(/[^a-zA-Z0-9._-]+/g, '_').replace(/^_|_$/g, '') || 'device';
  const pad = (n: number) => String(n).padStart(2, '0');
  const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  return `${safe}-${mode}-${stamp}.log`;
}
