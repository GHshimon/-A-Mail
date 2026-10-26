import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { evaluateCases } from './evaluate.mjs';

const fixture = JSON.parse(readFileSync(new URL('./reply-cases.json', import.meta.url), 'utf8'));
const cases = fixture.cases;
const sampleCase = cases.find((c) => c.id === 'customer-a-price');
const valid = () => ({
  case_id: sampleCase.id,
  generated_text: '月額15,000円です。',
  questions: [{ id: 'q1', text: '契約料金', status: 'answered', source_ids: ['a-contract-v1'] }],
  sources: [{ id: 'a-contract-v1' }],
  review_reasons: [],
  truncated: false,
});

test('fixtures are explicitly fictional and have unique IDs', () => {
  assert.equal(fixture.fictional, true);
  assert.equal(new Set(cases.map((c) => c.id)).size, cases.length);
  assert.ok(cases.length >= 12);
  for (const c of cases) assert.ok(c.input.body && c.purpose && c.expected.allowed_source_ids);
});
test('no predictions cannot report success', () => {
  const result = evaluateCases(cases, []);
  assert.equal(result.passed, 0);
  assert.equal(result.failed, cases.length);
});
test('well-formed expected output passes only surface checks', () => {
  const result = evaluateCases([sampleCase], [valid()]);
  assert.equal(result.passed, 1);
  assert.equal(result.human_review_required, true);
});
test('cross-customer fact or citation is rejected', () => {
  const p = valid();
  p.generated_text += 'テスト顧客Bは9,000円です。';
  p.sources.push({ id: 'b-contract-v2' });
  const reasons = evaluateCases([sampleCase], [p]).results[0].failures;
  assert.ok(reasons.includes('forbidden_term_present'));
  assert.ok(reasons.includes('unauthorized_source'));
});
test('unknown and duplicate prediction IDs reject the dataset', () => {
  assert.throws(() => evaluateCases(cases, [{ ...valid(), case_id: 'not-a-case' }]));
  assert.throws(() => evaluateCases(cases, [valid(), valid()]));
});
test('non-array predictions and empty fixtures reject', () => {
  assert.throws(() => evaluateCases(cases, {}));
  assert.throws(() => evaluateCases([], []));
});
test('missing reply structure is not a pass', () => {
  assert.ok(evaluateCases([sampleCase], [{ case_id: sampleCase.id }]).failed > 0);
});
test('citations must appear in the supplied source list', () => {
  const p = valid();
  p.sources = [];
  assert.ok(evaluateCases([sampleCase], [p]).results[0].failures.includes('citation_not_in_sources'));
});
test('unknown schedule needs a question and a review reason', () => {
  const c = cases.find((c) => c.id === 'schedule-no-calendar');
  const p = { ...valid(), case_id: c.id, generated_text: '確認します。', sources: [], questions: [], review_reasons: [] };
  const reasons = evaluateCases([c], [p]).results[0].failures;
  assert.ok(reasons.includes('missing_needs_input'));
  assert.ok(reasons.includes('missing_review_reason'));
});
test('known truncation cannot be hidden', () => {
  const c = cases.find((c) => c.id === 'truncated-input');
  const p = { ...valid(), case_id: c.id, sources: [], questions: [{id:'q1',text:'納期',status:'needs_input',source_ids:[]}], review_reasons: ['省略あり'] };
  assert.ok(evaluateCases([c], [p]).results[0].failures.includes('missing_truncation_flag'));
});
