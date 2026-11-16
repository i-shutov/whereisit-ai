import React, { createContext, useContext, useEffect, useState } from 'react';

export type Lang = 'ru' | 'en';

interface I18nContextValue {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (ru: string, en: string) => string;
}

const I18nContext = createContext<I18nContextValue>({
  lang: 'ru',
  setLang: () => {},
  t: (ru) => ru,
});

export const I18nProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [lang, setLangState] = useState<Lang>(() => {
    if (typeof window === 'undefined') return 'ru';
    const saved = window.localStorage.getItem('whereisit_lang');
    return saved === 'en' ? 'en' : 'ru';
  });

  useEffect(() => {
    document.documentElement.lang = lang;
    try {
      window.localStorage.setItem('whereisit_lang', lang);
    } catch {
      // ignore storage quota errors
    }
  }, [lang]);

  const setLang = (next: Lang) => setLangState(next);
  const t = (ru: string, en: string) => (lang === 'en' ? en : ru);

  return (
    <I18nContext.Provider value={{ lang, setLang, t }}>
      {children}
    </I18nContext.Provider>
  );
};

export function useI18n(): I18nContextValue {
  return useContext(I18nContext);
}
