// Accès au document : interface commune et document « zone de texte »
// (mode démonstration de l'add-in, éditeur de l'application de bureau).

import { type Target, replacement } from "./mapping";

/**
 * Résultat d'une action sur le document :
 * - `ok` : fait ;
 * - `changed` : le paragraphe a été modifié depuis l'analyse ;
 * - `notfound` : le texte n'a pas été retrouvé.
 */
export type ActionResult = "ok" | "changed" | "notfound";

export interface DocumentAccess {
  readonly kind: "word" | "editeur";
  readParagraphs(): Promise<string[]>;
  /** Sélectionne l'erreur dans le document. */
  select(paragraph: number, expected: string, target: Target): Promise<ActionResult>;
  /** Remplace l'erreur par `suggestion`. */
  replace(paragraph: number, expected: string, target: Target, suggestion: string): Promise<ActionResult>;
}

/** Zone de texte, un paragraphe par ligne. */
export class EditorDocument implements DocumentAccess {
  readonly kind = "editeur";

  constructor(private readonly area: HTMLTextAreaElement) {}

  async readParagraphs(): Promise<string[]> {
    return this.area.value.split("\n");
  }

  async select(paragraph: number, expected: string, target: Target): Promise<ActionResult> {
    const at = this.offset(paragraph, expected);
    if (at === null) return "changed";
    const start = at + target.start + target.prefix.length;
    const end = at + target.start + target.snippet.length - target.suffix.length;
    this.area.focus();
    this.area.setSelectionRange(start, end);
    return "ok";
  }

  async replace(paragraph: number, expected: string, target: Target, suggestion: string): Promise<ActionResult> {
    const at = this.offset(paragraph, expected);
    if (at === null) return "changed";
    const start = at + target.start;
    const end = start + target.snippet.length;
    const v = this.area.value;
    this.area.value = v.slice(0, start) + replacement(target, suggestion) + v.slice(end);
    return "ok";
  }

  /** Position du début du paragraphe, ou `null` s'il a changé. */
  private offset(paragraph: number, expected: string): number | null {
    const lines = this.area.value.split("\n");
    if (lines[paragraph] !== expected) return null;
    return lines.slice(0, paragraph).reduce((n, l) => n + l.length + 1, 0);
  }
}
