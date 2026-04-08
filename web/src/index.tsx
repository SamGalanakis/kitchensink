/* @refresh reload */
import { render } from 'solid-js/web'
import './index.css'
import App from './App.tsx'
import { registerHirselDocumentComponents } from './lib/hirsel-document-components'

const root = document.getElementById('root')

registerHirselDocumentComponents()

render(() => <App />, root!)
