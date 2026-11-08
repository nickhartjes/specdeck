// The part of Prism this app uses; `@types/prismjs` is not in the offline package store.
declare module 'prismjs/components/prism-core' {
  export interface Token {
    type: string;
    content: string | Token | (string | Token)[];
    alias?: string | string[];
  }
  export type Grammar = Record<string, unknown>;
  const Prism: {
    languages: Record<string, Grammar | undefined>;
    tokenize(text: string, grammar: Grammar): (string | Token)[];
  };
  export default Prism;
}

declare module 'prismjs/components/*';
