// What the Settings screen reads that must outlive it: TvStage remounts a
// tab whenever a page closes over it (the folder browser of Change library
// folder…), and the Library pane's row must come back enabled at once so
// focus returns to it.
export const settingsState = $state<{ libPath: string | null }>({ libPath: null });
