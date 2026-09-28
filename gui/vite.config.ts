import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// En desarrollo la API la sirve jmi-server (cargo run -p jmi-server); Vite le pasa /api.
export default defineConfig({
  plugins: [react()],
  server: { proxy: { '/api': process.env.JMI_API ?? 'http://127.0.0.1:8787' } },
});
