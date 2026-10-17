// Types partagés par l'add-in Word et l'application de bureau.

export type Category = "orthographe" | "homophone" | "grammaire" | "typographie" | "style";

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
  /** Phrase signalée comme longue au-delà de ce nombre de mots (style). */
  longSentenceWords: number;
  /** Distance maximale, en mots, d'une répétition signalée (style). */
  repetitionWindow: number;
}

export const DEFAULT_OPTIONS: CheckOptions = {
  // Le style (répétitions, phrases longues) relève du conseil : désactivé par défaut.
  disabledCategories: ["style"],
  disabledRules: [],
  userWords: [],
  narrowNbsp: true,
  typographicApostrophe: true,
  frenchQuotes: true,
  ellipsisCharacter: true,
  checkProperNouns: false,
  maxSuggestions: 5,
  longSentenceWords: 35,
  repetitionWindow: 40,
};

export interface Conjugation {
  infinitive: string;
  model: string;
  auxiliary: "avoir" | "être";
  pronominal: boolean;
  /** Verbe absent de la liste : modèle déduit de la terminaison. */
  guessed: boolean;
  modes: { name: string; tenses: { name: string; lines: string[] }[] }[];
}

export interface Synonyms {
  /** Mot consulté (le lemme si une forme fléchie a été demandée). */
  entry: string;
  groups: { category: string; words: string[] }[];
  /** Antonymes par catégorie (Wiktionnaire). */
  antonyms: { category: string; words: string[] }[];
}

export interface Readability {
  words: number;
  sentences: number;
  syllables: number;
  /** Indice de Kandel et Moles : plus il est élevé, plus le texte est facile. */
  score: number;
  level: string;
}

/** Moteur vu par l'interface : WebAssembly (Word) ou bibliothèque native (bureau). */
export interface Checker {
  /** Nombre de formes du lexique. */
  readonly forms: number;
  check(text: string, options: CheckOptions): Promise<Issue[]>;
  conjugate(verb: string): Promise<Conjugation | null>;
  synonyms(word: string): Promise<Synonyms | null>;
  readability(text: string): Promise<Readability>;
}
