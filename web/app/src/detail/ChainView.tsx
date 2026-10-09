// F7: a chain behind a derived date or a shortfall: its constraints in order, each with its
// source and labeled when conditional, the dates fixed on it, and the pins and rules to edit.
import { constraintText, editTargets, fixedText, namer, sourceText, targetText, type Chain } from "./explain.ts";
import type { Ready } from "./model.ts";

/** A chain: its constraints in order with their sources, the dates fixed on it, what to edit. */
export function ChainView({ view, chain }: { view: Ready; chain: Chain }) {
  const name = namer(view);
  const targets = editTargets(chain, view);
  return (
    <div className="stack chain" data-testid="chain">
      {chain.constraints.length === 0 ? null : (
        <ol className="detail-list">
          {chain.constraints.map((constraint, at) => (
            <li key={at} data-testid="chain-link">
              {constraintText(constraint, name)}
              <span className="muted small"> ({sourceText(constraint.source, name)})</span>
              {constraint.conditional === true ? <span className="badge warn">conditional</span> : null}
            </li>
          ))}
        </ol>
      )}
      <ul className="detail-list">
        {(chain.fixed ?? []).map((fixed, at) => (
          <li key={at} data-testid="chain-fixed">
            {fixedText(fixed, name)}
          </li>
        ))}
      </ul>
      <span className="muted small" data-testid="chain-edit">
        {targets.length === 0
          ? "Nothing here moves it: it rests on facts."
          : `To change it, edit ${targets.map((target) => targetText(target, name)).join("; ")}.`}
      </span>
    </div>
  );
}
