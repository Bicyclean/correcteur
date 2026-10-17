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
  /** Texte sélectionné (outil Synonymes). */
  selectedText(): Promise<string>;
  /** Remplace la sélection (choix d'un synonyme). */
  replaceSelection(text: string): Promise<void>;
}

/** Zone de texte, un paragraphe par ligne. */
export class EditorDocument implements DocumentAccess {
  readonly kind = "editeur";
  /** Vrai pendant une modification faite par le panneau (et non par la frappe). */
  programmatic = false;

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
    this.insert(start, start + target.snippet.length, replacement(target, suggestion));
    return "ok";
  }

  /**
   * Remplace une zone par l'édition native du navigateur, qui garde
   * l'historique : ⌘Z / Ctrl+Z annule la correction comme une frappe.
   */
  private insert(start: number, end: number, text: string): void {
    const area = this.area;
    const scroll = area.scrollTop;
    area.focus({ preventScroll: true });
    area.setSelectionRange(start, end);
    this.programmatic = true;
    try {
      const native = text === "" ? document.execCommand("delete") : document.execCommand("insertText", false, text);
      if (!native) {
        area.value = area.value.slice(0, start) + text + area.value.slice(end);
      }
    } finally {
      this.programmatic = false;
    }
    area.setSelectionRange(start + text.length, start + text.length);
    area.scrollTop = scroll;
  }

  /** Annule la dernière modification (historique natif de la zone de texte). */
  undo(): void {
    this.area.focus({ preventScroll: true });
    this.programmatic = true;
    try {
      document.execCommand("undo");
    } finally {
      this.programmatic = false;
    }
  }

  async selectedText(): Promise<string> {
    return this.area.value.slice(this.area.selectionStart, this.area.selectionEnd);
  }

  async replaceSelection(text: string): Promise<void> {
    const { selectionStart: s, selectionEnd: e } = this.area;
    this.insert(s, e, text);
    this.area.setSelectionRange(s, s + text.length);
  }

  /** Position du début du paragraphe, ou `null` s'il a changé. */
  private offset(paragraph: number, expected: string): number | null {
    const lines = this.area.value.split("\n");
    if (lines[paragraph] !== expected) return null;
    return lines.slice(0, paragraph).reduce((n, l) => n + l.length + 1, 0);
  }
}
