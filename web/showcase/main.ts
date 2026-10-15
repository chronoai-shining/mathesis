import './style.css';

const application = import.meta.env.VITE_MATHESIS_APP_URL as string | undefined;
if (application) {
  try {
    const url = new URL(application);
    if (url.protocol === 'https:' && !url.username && !url.password) {
      const link = document.querySelector<HTMLAnchorElement>('[data-application]');
      if (link) {
        link.href = url.href;
        link.hidden = false;
      }
    }
  } catch {
    console.error('The application URL is invalid.');
  }
}
