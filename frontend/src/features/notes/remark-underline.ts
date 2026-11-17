/**
 * `++underline++` for markdown (pandoc-style), via a micromark syntax
 * extension + an mdast handler mapping to a `u` element.
 *
 * Why hand-rolled: `__` is taken by emphasis, raw HTML stays escaped by
 * design, and the built-in CommonMark set has no underline. The extension
 * mirrors GFM strikethrough's design (doubled `++` markers, open/close
 * resolution with the same punctuation rules), so nested constructs inside
 * the span work (`++**bold**++`, `++*em* `code`++`).
 *
 * The resulting mdast node is unknown to mdast-util-to-hast; `data.hName`
 * makes it round-trip into a `<u>` element (see `state.js#applyData` /
 * `defaultUnknownHandler`, which rename the placeholder element to `u`).
 * The inline renderer (`cards/markdown-preview.tsx`) intentionally does NOT
 * support nesting — this extension is what the full renderer uses.
 */
import { classifyCharacter } from 'micromark-util-classify-character';
import { splice } from 'micromark-util-chunked';
import { resolveAll } from 'micromark-util-resolve-all';
import type {
  Code,
  Construct,
  Effects,
  Event,
  Extension,
  Resolver,
  State,
  TokenizeContext,
} from 'micromark-util-types';
import type { Nodes } from 'mdast';
import type {
  CompileContext,
  Extension as FromMarkdownExtension,
} from 'mdast-util-from-markdown';
import type { Plugin } from 'unified';

declare module 'micromark-util-types' {
  interface TokenTypeMap {
    underline: 'underline';
    underlineText: 'underlineText';
    underlineSequence: 'underlineSequence';
    underlineSequenceTemporary: 'underlineSequenceTemporary';
  }
}

const PLUS: Code = 43; // '+'

function underlineSyntax(): Extension {
  const tokenizer: Construct = {
    name: 'underline',
    tokenize: tokenizeUnderline,
    resolveAll: resolveAllUnderline,
  };
  return {
    text: { '43': tokenizer },
    insideSpan: { null: [{ resolveAll: resolveAllUnderline }] },
    attentionMarkers: { null: [PLUS] },
  };
}

const resolveAllUnderline: Resolver = (events, context) => {
  let index = -1;

  // Walk through all events.
  while (++index < events.length) {
    // Find a token that can close.
    if (
      events[index][0] === 'enter' &&
      events[index][1].type === 'underlineSequenceTemporary' &&
      events[index][1]._close
    ) {
      let open = index;

      // Now walk back to find an opener.
      while (open--) {
        // Find a token that can open the closer.
        if (
          events[open][0] === 'exit' &&
          events[open][1].type === 'underlineSequenceTemporary' &&
          events[open][1]._open &&
          // If the sizes are the same:
          events[index][1].end.offset - events[index][1].start.offset ===
            events[open][1].end.offset - events[open][1].start.offset
        ) {
          events[index][1].type = 'underlineSequence';
          events[open][1].type = 'underlineSequence';

          const underline: (typeof events)[number][1] = {
            type: 'underline',
            start: Object.assign({}, events[open][1].start),
            end: Object.assign({}, events[index][1].end),
          };

          const text: (typeof events)[number][1] = {
            type: 'underlineText',
            start: Object.assign({}, events[open][1].end),
            end: Object.assign({}, events[index][1].start),
          };

          // Opening.
          const nextEvents: Array<Event> = [
            ['enter', underline, context],
            ['enter', events[open][1], context],
            ['exit', events[open][1], context],
            ['enter', text, context],
          ];
          const insideSpan = context.parser.constructs.insideSpan?.null;
          if (insideSpan) {
            // Between.
            splice(
              nextEvents,
              nextEvents.length,
              0,
              resolveAll(insideSpan, events.slice(open + 1, index), context),
            );
          }

          // Closing.
          splice(
            nextEvents,
            nextEvents.length,
            0,
            [
              ['exit', text, context],
              ['enter', events[index][1], context],
              ['exit', events[index][1], context],
              ['exit', underline, context],
            ],
          );
          splice(events, open - 1, index - open + 3, nextEvents);
          index = open + nextEvents.length - 2;
          break;
        }
      }
    }
  }

  index = -1;
  while (++index < events.length) {
    if (events[index][1].type === 'underlineSequenceTemporary') {
      events[index][1].type = 'data';
    }
  }
  return events;
};

function tokenizeUnderline(
  this: TokenizeContext,
  effects: Effects,
  ok: State,
  nok: State,
): State {
  const previous = this.previous;
  const events = this.events;
  let size = 0;
  return start;

  function start(code: Code): State | undefined {
    if (previous === PLUS && events[events.length - 1][1].type !== 'characterEscape') {
      return nok(code);
    }
    effects.enter('underlineSequenceTemporary');
    return more(code);
  }

  function more(code: Code): State | undefined {
    const before = classifyCharacter(previous);
    if (code === PLUS) {
      // If this is the third marker, exit.
      if (size >= 2) return nok(code);
      effects.consume(code);
      size++;
      return more;
    }
    if (size < 2) return nok(code);
    const token = effects.exit('underlineSequenceTemporary');
    const after = classifyCharacter(code);
    token._open = !after || (after === 2 && Boolean(before));
    token._close = !before || (before === 2 && Boolean(after));
    return ok(code);
  }
}

/** mdast extension: maps the `underline` token pair onto an unknown node carrying hName `u`. */
const underlineFromMarkdown: FromMarkdownExtension = {
  canContainEols: ['underline'],
  enter: {
    underline(this: CompileContext, token) {
      this.enter(
        {
          type: 'underline',
          children: [],
          data: { hName: 'u' },
        } as unknown as Nodes,
        token,
      );
    },
  },
  exit: {
    underline(this: CompileContext, token) {
      this.exit(token);
    },
  },
};

/** remark plugin — registers the micromark + mdast extensions. */
export const remarkUnderline: Plugin = function () {
  const self = this as unknown as {
    data(): Record<string, unknown> & {
      micromarkExtensions?: unknown[];
      fromMarkdownExtensions?: unknown[];
    };
  };
  const data = self.data();

  const micromarkExtensions =
    data.micromarkExtensions || (data.micromarkExtensions = []);
  const fromMarkdownExtensions =
    data.fromMarkdownExtensions || (data.fromMarkdownExtensions = []);

  micromarkExtensions.push(underlineSyntax());
  fromMarkdownExtensions.push(underlineFromMarkdown);
};
