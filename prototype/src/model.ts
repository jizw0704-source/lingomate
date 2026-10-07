export interface Translation {
  word: string;
  pos: string;
  note: string;
  example: string;
}
export interface Candidate {
  text: string;
  syllables: string[];
  translations: Translation[];
  hasDetails: boolean;
}
export interface EngineResult {
  input: string;
  marked: string;
  candidates: Candidate[];
  committed: string | null;
}

export function normalizePinyin(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z']/g, "")
    .slice(0, 80);
}

/** Insert at the actual editor selection. Add spacing only at English word boundaries. */
export function insertText(
  value: string,
  start: number,
  end: number,
  text: string,
) {
  const before = value.slice(0, start);
  const after = value.slice(end);
  const leading =
    /[a-z0-9]$/i.test(before) && /^[a-z0-9]/i.test(text) ? " " : "";
  const trailing =
    /[a-z0-9]$/i.test(text) && /^[a-z0-9]/i.test(after) ? " " : "";
  const inserted = leading + text + trailing;
  return { value: before + inserted + after, cursor: start + inserted.length };
}
