// Whether a model error is something Settings can fix (a refused key, an
// account with no credit, no model set up), so it's worth linking there.
// Timeouts and odd replies aren't: Settings would be a dead end.
export const settingsCanFix = (message: string) =>
  /key|account|credit|quota|billing|no model is set up|isn't in config|connect one in Settings/i.test(message);
