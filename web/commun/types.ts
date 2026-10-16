// Types partagés par l'add-in Word et l'application de bureau.

export type Category = "orthographe" | "homophone" | "grammaire" | "typographie";

export interface Issue {
  /** Début, en unités UTF-16 dans le texte analysé. */
  start: number;
  end: number;
  rule: string;
  category: Category;
  message: string;
  explanation: string;
  suggestions: string[];
}

export interface CheckOptions {
  disabledCategories: Category[];
  disabledRules: string[];
  userWords: string[];
  narrowNbsp: boolean;
  typographicApostrophe: boolean;
  frenchQuotes: boolean;
  ellipsisCharacter: boolean;
  checkProperNouns: boolean;
  maxSuggestions: number;
}

export const DEFAULT_OPTIONS: CheckOptions = {
  disabledCategories: [],
  disabledRules: [],
  userWords: [],
  narrowNbsp: true,
  typographicApostrophe: true,
  frenchQuotes: true,
  ellipsisCharacter: true,
  checkProperNouns: false,
  maxSuggestions: 5,
};

/** Moteur vu par l'interface : WebAssembly (Word) ou bibliothèque native (bureau). */
export interface Checker {
  /** Nombre de formes du lexique. */
  readonly forms: number;
  check(text: string, options: CheckOptions): Promise<Issue[]>;
}
