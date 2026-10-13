import { render } from '@testing-library/react';
import { expect, it } from 'vitest';
import { InlineLatex } from './Markdown';
it('renders title math safely inside headings and links', () => {
  const { container } = render(
    <h1>
      <a href="/paper">
        <InlineLatex
          source={'Zero forcing of $P(n,4)$ [link](javascript:alert(1)) <script>bad()</script>'}
        />
      </a>
    </h1>,
  );
  expect(container.querySelector('.katex')).not.toBeNull();
  expect(container.querySelectorAll('a')).toHaveLength(1);
  expect(container.querySelector('h1 p')).toBeNull();
  expect(container.querySelector('script')).toBeNull();
});
