// Passage d'une position dans le texte d'un paragraphe à une plage Word.
//
// Office.js ne permet pas d'obtenir une plage à partir d'une position : on
// cherche un extrait de texte dans le paragraphe (`paragraph.search`) et on
// retient la bonne occurrence. Ce module calcule l'extrait et le rang de
// l'occurrence ; il ne dépend pas de Word et se teste dans Node.

/** Zone d'un paragraphe, en unités UTF-16 (positions des chaînes JavaScript). */
export interface Span {
  start: number;
  end: number;
}

/** Ce qu'il faut chercher dans le paragraphe pour retrouver une erreur. */
export interface Target {
  /** Texte recherché : la zone de l'erreur, élargie si besoin. */
  snippet: string;
  /** Rang de l'extrait parmi ses occurrences exactes dans le paragraphe (0 = première). */
  occurrence: number;
  /** Début de l'extrait dans le paragraphe. */
  start: number;
  /** Texte de l'extrait situé avant la zone de l'erreur. */
  prefix: string;
  /** Texte de l'extrait situé après la zone de l'erreur. */
  suffix: string;
}

/** Longueur maximale d'une chaîne de recherche dans Word. */
const MAX_SEARCH = 255;

const isSpace = (c: string | undefined): boolean => c !== undefined && /\s/u.test(c);
const isLowSurrogate = (code: number): boolean => code >= 0xdc00 && code <= 0xdfff;

/**
 * Calcule l'extrait à chercher pour la zone `span` du paragraphe `text`.
 *
 * Word ignore les recherches faites uniquement de blancs et tolère mal les
 * blancs en bord de chaîne : l'extrait est élargi jusqu'à commencer et finir
 * par un caractère visible (« venez ? » pour l'espace avant « ? »). Si
 * l'extrait n'est pas trouvable tel quel (occurrence chevauchant une
 * précédente), il est élargi d'un caractère de chaque côté.
 */
export function locate(text: string, span: Span): Target | null {
  if (span.start < 0 || span.end > text.length || span.start > span.end) return null;
  let s = span.start;
  let e = span.end;
  // Extrait vide (insertion) : prendre au moins un caractère voisin.
  if (s === e) {
    if (e < text.length) e++;
    else if (s > 0) s--;
    else return null;
  }
  while (s > 0 && isSpace(text[s])) s--;
  while (e < text.length && isSpace(text[e - 1])) e++;
  // Ne jamais couper une paire de substitution (émoji…).
  const fix = () => {
    if (s > 0 && isLowSurrogate(text.charCodeAt(s))) s--;
    if (e < text.length && isLowSurrogate(text.charCodeAt(e))) e++;
  };
  fix();

  for (;;) {
    if (e - s > MAX_SEARCH) return null;
    const snippet = text.slice(s, e);
    const occurrence = searchRank(text, snippet, s);
    if (occurrence !== null) {
      return {
        snippet,
        occurrence,
        start: s,
        prefix: text.slice(s, span.start),
        suffix: text.slice(span.end, e),
      };
    }
    if (s === 0 && e === text.length) return null;
    if (s > 0) s--;
    if (e < text.length) e++;
    fix();
  }
}

/**
 * Rang de l'occurrence commençant en `at` parmi les occurrences trouvées de
 * gauche à droite sans chevauchement, comme le fait la recherche de Word ;
 * `null` si cette occurrence serait sautée.
 */
export function searchRank(text: string, snippet: string, at: number): number | null {
  let rank = 0;
  let from = 0;
  for (;;) {
    const i = text.indexOf(snippet, from);
    if (i === -1 || i > at) return null;
    if (i === at) return rank;
    rank++;
    from = i + snippet.length;
  }
}

/** Échappe les codes spéciaux de la recherche Word (`^`). */
export function escapeSearch(snippet: string): string {
  return snippet.replace(/\^/g, "^^");
}

/** Texte qui remplace l'extrait entier pour appliquer `suggestion`. */
export function replacement(target: Target, suggestion: string): string {
  return target.prefix + suggestion + target.suffix;
}

/** Texte du paragraphe après application de la suggestion (pour vérifier ou simuler). */
export function applyToText(text: string, span: Span, suggestion: string): string {
  return text.slice(0, span.start) + suggestion + text.slice(span.end);
}
