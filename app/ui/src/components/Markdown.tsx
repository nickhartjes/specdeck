import { useContext, useState, type ComponentPropsWithoutRef, type KeyboardEvent, type MouseEvent } from 'react';
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

/** `url` without the spaces and control characters browsers ignore in a scheme, so none hides one. */
const squeezed = (url: string) => [...url].filter((c) => c > ' ').join('');

/**
 * Keeps a link's address as written, so it can be shown even when it will not open (a `file:`
 * address, say); only addresses that could run script are dropped. `ContentLink` decides what
 * becomes a navigable `href`.
 */
function keepAddress(url: string, key: string): string {
  if (key !== 'href') return defaultUrlTransform(url);
  return SCRIPTED.test(squeezed(url)) ? '' : url;
}

/** Whether `href` is a web address, the only kind an anchor may carry as its `href`. */
function web(href: string): boolean {
  return /^https?:/i.test(squeezed(href));
}

/** Whether `href` names an address with a scheme, rather than a place in this page or a relative path. */
function absolute(href: string): boolean {
  return /^[a-z][a-z0-9+.-]*:/i.test(href);
}

/**
 * A link in session content (`confine-window-navigation` design decision 2). The window never
 * follows it: a click or a middle-click asks the app to open it in the system browser, which it
 * does only for web addresses that pass the check for shared links, and says so when it refuses.
 * Only a web address is put in `href`, so what the browser itself might follow ("Open in new
 * tab") is never another kind of address; others show their address as a title only.
 */
function ContentLink({ href = '', children, ...props }: ComponentPropsWithoutRef<'a'> & { node?: unknown }) {
  // react-markdown's syntax tree node, not an attribute.
  delete props.node;
  const bridge = useContext(BridgeContext);
  const [refusal, setRefusal] = useState<string | null>(null);
  const open = () => {
    if (!bridge || !absolute(href)) return;
    setRefusal(null);
    bridge.openContentLink(href).catch((error: unknown) => setRefusal(errorMessage(error)));
  };
  const click = (event: MouseEvent<HTMLAnchorElement>) => {
    event.preventDefault();
    open();
  };
  // A middle-click fires `auxclick`, not `click`.
  const auxClick = (event: MouseEvent<HTMLAnchorElement>) => {
    if (event.button !== 1) return;
    event.preventDefault();
    open();
  };
  const navigable = web(href);
  // Without an `href` an anchor is not a link to the keyboard; Enter still asks.
  const key = (event: KeyboardEvent<HTMLAnchorElement>) => {
    if (navigable || event.key !== 'Enter') return;
    event.preventDefault();
    open();
  };
  return (
    <>
      <a
        {...props}
        href={navigable ? href : undefined}
        role={navigable ? undefined : 'link'}
        tabIndex={navigable ? undefined : 0}
        title={href || undefined}
        rel="noopener noreferrer"
        referrerPolicy="no-referrer"
        onClick={click}
        onAuxClick={auxClick}
        onKeyDown={key}
      >
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
