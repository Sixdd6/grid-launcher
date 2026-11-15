// What is left of Settings › Connection, which the RomM card on Settings ›
// Game stores replaced (romm-storefront-plan Step 4). Only TV mode's
// Connection pane still reads this; it goes with that pane (Step 6).

export function serverLine(serverUrl: string): string {
  const trimmed = serverUrl.trim();
  return trimmed === '' ? 'Not set' : trimmed;
}
