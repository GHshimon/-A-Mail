import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const normalize = (value) => value.normalize('NFKC');

/** Deterministic surface checks only; not semantic or personal-style grading. */
export function evaluateCases(cases, predictions) {
  if (!Array.isArray(cases) || cases.length === 0) throw new Error('評価ケースがありません');
  if (!Array.isArray(predictions)) throw new Error('予測はJSON配列で指定してください');
  const known = new Set(cases.map((item) => item.id));
  if (known.size !== cases.length) throw new Error('評価ケースIDが重複しています');
  const byId = new Map();
  for (const prediction of predictions) {
    if (!prediction || typeof prediction.case_id !== 'string' || !known.has(prediction.case_id)) {
      throw new Error('未知または不正な評価ケースIDです');
    }
    if (byId.has(prediction.case_id)) throw new Error('予測の評価ケースIDが重複しています');
    byId.set(prediction.case_id, prediction);
  }
  const results = cases.map(({ id, expected }) => {
    const p = byId.get(id);
    const failures = [];
    if (!p) return { case_id: id, pass: false, failures: ['missing_prediction'] };
    if (typeof p.generated_text !== 'string' || !p.generated_text.trim()) failures.push('missing_text');
    if (!Array.isArray(p.questions)) failures.push('invalid_questions');
    if (!Array.isArray(p.sources)) failures.push('invalid_sources');
    if (!Array.isArray(p.review_reasons) || p.review_reasons.some((s) => typeof s !== 'string')) failures.push('invalid_review_reasons');
    if (typeof p.truncated !== 'boolean') failures.push('invalid_truncated');
    const body = typeof p.generated_text === 'string' ? normalize(p.generated_text) : '';
    if (expected.required_terms.some((term) => !body.includes(normalize(term)))) failures.push('required_term_missing');
    if (expected.forbidden_terms.some((term) => body.includes(normalize(term)))) failures.push('forbidden_term_present');
    const questions = Array.isArray(p.questions) ? p.questions : [];
    if (questions.length < expected.min_questions) failures.push('question_coverage_count');
    if (questions.filter((q) => q?.status === 'needs_input').length < expected.min_needs_input) failures.push('missing_needs_input');
    const allowedSources = new Set(expected.allowed_source_ids);
    const cited = [];
    const ids = new Set();
    for (const q of questions) {
      if (!q || typeof q.id !== 'string' || !q.id || typeof q.text !== 'string' || !q.text.trim() || !['answered', 'needs_input'].includes(q.status) || !Array.isArray(q.source_ids)) {
        failures.push('invalid_question');
        continue;
      }
      if (ids.has(q.id)) failures.push('duplicate_question_id');
      ids.add(q.id);
      cited.push(...q.source_ids);
    }
    for (const source of Array.isArray(p.sources) ? p.sources : []) cited.push(source?.id);
    if (cited.some((id) => typeof id !== 'string' || !allowedSources.has(id))) failures.push('unauthorized_source');
    const available = new Set((Array.isArray(p.sources) ? p.sources : []).map((s) => s?.id));
    if (questions.some((q) => Array.isArray(q?.source_ids) && q.source_ids.some((id) => !available.has(id)))) failures.push('citation_not_in_sources');
    if (expected.requires_review && (!Array.isArray(p.review_reasons) || !p.review_reasons.some((s) => typeof s === 'string' && s.trim()))) failures.push('missing_review_reason');
    if (expected.requires_truncated && p.truncated !== true) failures.push('missing_truncation_flag');
    return { case_id: id, pass: failures.length === 0, failures: [...new Set(failures)] };
  });
  return {
    kind: 'offline_surface_checks_not_semantic_accuracy',
    total: cases.length,
    passed: results.filter((r) => r.pass).length,
    failed: results.filter((r) => !r.pass).length,
    human_review_required: true,
    results,
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 3) throw new Error('使用法: node evals/evaluate.mjs <架空データの予測JSON>');
    const fixture = JSON.parse(readFileSync(new URL('./reply-cases.json', import.meta.url), 'utf8'));
    const predictions = JSON.parse(readFileSync(process.argv[2], 'utf8'));
    const result = evaluateCases(fixture.cases, predictions);
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    if (result.failed > 0) process.exitCode = 1;
  } catch (error) {
    // Do not print the input, path, generated text, or native parser message.
    process.stderr.write('評価できません。入力形式・件数・ケースIDを確認してください。\n');
    process.exitCode = 2;
  }
}
