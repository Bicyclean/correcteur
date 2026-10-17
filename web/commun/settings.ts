// Réglages de l'utilisateur, conservés dans le stockage local du panneau.
// Rien n'est envoyé sur le réseau.

import { type CheckOptions, DEFAULT_OPTIONS } from "./types";

const KEY = "correcteur.reglages.v1";

export function loadSettings(): CheckOptions {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return structuredClone(DEFAULT_OPTIONS);
    return sanitize(JSON.parse(raw));
  } catch {
    return structuredClone(DEFAULT_OPTIONS);
  }
}

export function saveSettings(o: CheckOptions): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(o));
  } catch {
    // Stockage indisponible (navigation privée…) : les réglages valent pour la session.
  }
}

/** Ne garde que des valeurs du bon type, pour résister à un stockage corrompu. */
export function sanitize(value: unknown): CheckOptions {
  const o = structuredClone(DEFAULT_OPTIONS);
  if (typeof value !== "object" || value === null) return o;
  const v = value as Record<string, unknown>;
  const bool = (k: keyof CheckOptions) => {
    if (typeof v[k] === "boolean") (o[k] as boolean) = v[k] as boolean;
  };
  const strings = (k: "disabledRules" | "userWords") => {
    if (Array.isArray(v[k])) o[k] = (v[k] as unknown[]).filter((x): x is string => typeof x === "string");
  };
  bool("narrowNbsp");
  bool("typographicApostrophe");
  bool("frenchQuotes");
  bool("ellipsisCharacter");
  bool("checkProperNouns");
  strings("disabledRules");
  strings("userWords");
  if (Array.isArray(v.disabledCategories)) {
    o.disabledCategories = v.disabledCategories.filter(
      (c): c is CheckOptions["disabledCategories"][number] =>
        c === "orthographe" ||
        c === "homophone" ||
        c === "grammaire" ||
        c === "typographie" ||
        c === "style",
    );
  }
  const integer = (k: "maxSuggestions" | "longSentenceWords" | "repetitionWindow", min: number, max: number) => {
    const x = v[k];
    if (typeof x === "number" && x >= min && x <= max) o[k] = Math.floor(x);
  };
  integer("maxSuggestions", 0, 20);
  integer("longSentenceWords", 10, 200);
  integer("repetitionWindow", 2, 500);
  return o;
}
