// Enveloppe TypeScript du module WebAssembly (crates/correcteur-wasm).
// Fonctionne dans le navigateur (panneau Word) comme dans Node (tests).

import {
  type CheckOptions,
  type Conjugation,
  DEFAULT_OPTIONS,
  type Issue,
  type Readability,
  type Synonyms,
} from "../../../web/commun/types";

export type { CheckOptions, Issue } from "../../../web/commun/types";
export { DEFAULT_OPTIONS } from "../../../web/commun/types";

interface Exports {
  memory: WebAssembly.Memory;
  cr_alloc(len: number): number;
  cr_free(ptr: number, len: number): void;
  cr_output_ptr(): number;
  cr_output_len(): number;
  cr_add_file(pathPtr: number, pathLen: number, contentPtr: number, contentLen: number): number;
  cr_build(): number;
  cr_check(textPtr: number, textLen: number, optPtr: number, optLen: number): number;
  cr_conjugate(ptr: number, len: number): number;
  cr_synonyms(ptr: number, len: number): number;
  cr_readability(ptr: number, len: number): number;
}

/** Format `clé=valeur` attendu par `cr_check`. */
export function encodeOptions(o: CheckOptions): string {
  const flag = (b: boolean) => (b ? "1" : "0");
  const lines = [
    `espace_fine=${flag(o.narrowNbsp)}`,
    `apostrophe=${flag(o.typographicApostrophe)}`,
    `guillemets=${flag(o.frenchQuotes)}`,
    `points=${flag(o.ellipsisCharacter)}`,
    `noms_propres=${flag(o.checkProperNouns)}`,
    `suggestions_max=${o.maxSuggestions}`,
    `phrase_longue=${o.longSentenceWords}`,
    `repetition=${o.repetitionWindow}`,
    ...o.disabledCategories.map((c) => `categorie_off=${c}`),
    ...o.disabledRules.map((r) => `regle_off=${r}`),
    // Un mot ne peut pas contenir de saut de ligne : il casserait le format.
    ...o.userWords.filter((w) => w && !/[\r\n=]/.test(w)).map((w) => `mot=${w}`),
  ];
  return lines.join("\n");
}

export class Engine {
  private constructor(
    private readonly w: Exports,
    /** Nombre de formes du lexique. */
    readonly forms: number,
  ) {}

  /**
   * Instancie le module et charge les fichiers de données.
   * `files` associe un chemin relatif à `data/` (« lexique/10-lexique383.tsv ») à son contenu.
   */
  static async load(wasm: BufferSource, files: Record<string, string>): Promise<Engine> {
    const { instance } = await WebAssembly.instantiate(wasm, {});
    const w = instance.exports as unknown as Exports;
    const call = (fn: () => number) => {
      if (fn() !== 0) throw new Error(readOutput(w));
    };
    for (const [path, content] of Object.entries(files)) {
      withStrings(w, [path, content], ([p, c]) =>
        call(() => w.cr_add_file(p.ptr, p.len, c.ptr, c.len)),
      );
    }
    call(() => w.cr_build());
    const { forms } = JSON.parse(readOutput(w)) as { forms: number };
    return new Engine(w, forms);
  }

  /** Appelle une fonction du module qui prend une chaîne et renvoie du JSON. */
  private query<T>(fn: "cr_conjugate" | "cr_synonyms" | "cr_readability", input: string): T {
    const w = this.w;
    return withStrings(w, [input], ([b]) => {
      if (w[fn](b.ptr, b.len) !== 0) throw new Error(readOutput(w));
      return JSON.parse(readOutput(w)) as T;
    });
  }

  conjugate(verb: string): Conjugation | null {
    return this.query("cr_conjugate", verb);
  }

  synonyms(word: string): Synonyms | null {
    return this.query("cr_synonyms", word);
  }

  readability(text: string): Readability {
    return this.query("cr_readability", text);
  }

  check(text: string, options: CheckOptions = DEFAULT_OPTIONS): Issue[] {
    const w = this.w;
    return withStrings(w, [text, encodeOptions(options)], ([t, o]) => {
      if (w.cr_check(t.ptr, t.len, o.ptr, o.len) !== 0) throw new Error(readOutput(w));
      return JSON.parse(readOutput(w)) as Issue[];
    });
  }
}

interface Buf {
  ptr: number;
  len: number;
}

function withStrings<T>(w: Exports, strings: string[], fn: (bufs: Buf[]) => T): T {
  const encoder = new TextEncoder();
  const bufs: Buf[] = strings.map((s) => {
    const bytes = encoder.encode(s);
    const ptr = w.cr_alloc(bytes.length);
    // La mémoire peut avoir grandi : relire `memory.buffer` après chaque allocation.
    new Uint8Array(w.memory.buffer, ptr, bytes.length).set(bytes);
    return { ptr, len: bytes.length };
  });
  try {
    return fn(bufs);
  } finally {
    for (const b of bufs) w.cr_free(b.ptr, b.len);
  }
}

function readOutput(w: Exports): string {
  return new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.cr_output_ptr(), w.cr_output_len()));
}
