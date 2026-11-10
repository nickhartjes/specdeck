import { useContext, useState, type ComponentPropsWithoutRef, type MouseEvent } from 'react';
import ReactMarkdown, { defaultUrlTransform } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { BridgeContext, errorMessage } from '../api/bridge';

interface MdNode {
  type: string;
  value?: string;
  children?: MdNode[];
}

/** Drops HTML comments, such as the notes in OpenSpec templates; other HTML stays visible as text. */
function remarkDropComments() {
  const drop = (node: MdNode) => {
    if (!node.children) return;
    node.children = node.children.filter((child) => !(child.type === 'html' && child.value?.trim().startsWith('<!--')));
    node.children.forEach(drop);
  };
  return drop;
}

/** Addresses that could run script if anything followed them; react-markdown's default drops them. */
const SCRIPTED = /^(javascript|vbscript|data):/i;

/**
 * Keeps a link's address as written, so it shows on hover and can be copied, even when it will not
 * open (a `file:` address, say); only addresses that could run script are dropped. Nothing follows
 * the address itself: `ContentLink` opens links.
 */
function keepAddress(url: string, key: string): string {
  if (key !== 'href') return defaultUrlTransform(url);
  // Browsers ignore spaces and control characters in a scheme, so they don't hide one here either.
  const squeezed = [...url].filter((c) => c > ' ').join('');
  return SCRIPTED.test(squeezed) ? '' : url;
}

/** Whether `href` names an address with a scheme, rather than a place in this page or a relative path. */
function absolute(href: string): boolean {
  return /^[a-z][a-z0-9+.-]*:/i.test(href);
}

/**
 * A link in session content (`confine-window-navigation` design decision 2). The window never
 * follows it: a click asks the app to open it in the system browser, which it does only for web
 * addresses that pass the check for shared links, and says so when it refuses.
 */
function ContentLink({ href = '', children, ...props }: ComponentPropsWithoutRef<'a'> & { node?: unknown }) {
  // react-markdown's syntax tree node, not an attribute.
  delete props.node;
  const bridge = useContext(BridgeContext);
  const [refusal, setRefusal] = useState<string | null>(null);
  const click = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    if (!bridge || !absolute(href)) return;
    setRefusal(null);
    bridge.openContentLink(href).catch((error: unknown) => setRefusal(errorMessage(error)));
  };
  return (
    <>
      <a {...props} href={href} title={href || undefined} onClick={click}>
        {children}
      </a>
      {refusal && (
        <span className="content-link-refused" role="status">
          {' '}
          {refusal}
        </span>
      )}
    </>
  );
}

/** Agent text and shared Markdown, formatted. Links open in the system browser, never in the window. */
export function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      <ReactMarkdown remarkPlugins={[remarkGfm, remarkDropComments]} urlTransform={keepAddress} components={{ a: ContentLink }}>
        {text}
      </ReactMarkdown>
    </div>
  );
}
