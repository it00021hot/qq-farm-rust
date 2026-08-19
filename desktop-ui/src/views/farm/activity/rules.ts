export type ActivityRulesDto = {
  title?: string;
  paragraphs?: string[];
};

export function normalizeActivityRules(raw: unknown): ActivityRulesDto | null {
  if (!raw || typeof raw !== 'object') return null;
  const row = raw as Record<string, unknown>;
  const paragraphs = Array.isArray(row.paragraphs)
    ? row.paragraphs.map(item => String(item).trim()).filter(Boolean)
    : [];
  if (!paragraphs.length) return null;
  const title = String(row.title || '').trim();
  return { title: title || undefined, paragraphs };
}
