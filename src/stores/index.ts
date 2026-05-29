import { defineStore } from 'pinia';
import { ref, computed } from 'vue';

const STORAGE_KEY = 'yulin_session';

function loadSession(): Session | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as Session;
  } catch {
    return null;
  }
}

function saveSession(session: Session | null) {
  if (session) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(session));
  } else {
    localStorage.removeItem(STORAGE_KEY);
  }
}

interface Session {
  id: number;
  username: string;
  displayName: string;
  role: 'employee' | 'boss' | 'admin';
  token: string;
}

export const useAppStore = defineStore('app', () => {
  const session = ref<Session | null>(loadSession());

  const isLoggedIn = computed(() => session.value !== null);
  const userId = computed(() => session.value?.id ?? 0);
  const username = computed(() => session.value?.username ?? '');
  const displayName = computed(() => session.value?.displayName ?? '');
  const userRole = computed(() => session.value?.role ?? 'employee');
  const token = computed(() => session.value?.token ?? '');

  function login(user: { id: number; username: string; display_name: string; role: string; token: string }) {
    const s: Session = {
      id: user.id,
      username: user.username,
      displayName: user.display_name,
      role: user.role as Session['role'],
      token: user.token,
    };
    session.value = s;
    saveSession(s);
  }

  function logout() {
    session.value = null;
    saveSession(null);
  }

  return { session, isLoggedIn, userId, username, displayName, userRole, token, login, logout };
});
