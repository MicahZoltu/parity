// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

import ReactDOM from 'react-dom';
import React from 'react';
import { BrowserRouter as Router, Redirect, Route } from 'react-router-dom';

/** Additional Frameworks **/
import 'bootstrap/dist/css/bootstrap.css';

/** Initialization **/
import injectTapEventPlugin from 'react-tap-event-plugin';
import ContextProvider from '@parity/ui/ContextProvider';
import { initStore } from '@parity/shared/redux';
import { api } from './parity';
const store = initStore(api);

injectTapEventPlugin();

/** Components **/
import SettingsHome from './Home';
import SettingsParity from './Node';
import SettingsProxy from './Proxy';
import Settings from './settings';

/** Stylesheets **/
import './index.css';

ReactDOM.render(
  <ContextProvider api={ api } store={ store }>
    <Router>
      <Settings>
        <Redirect from='/settings.html' to='/home' />
        <Route path='/home' component={ SettingsHome } />
        <Route path='/parity' component={ SettingsParity } />
        <Route path='/proxy' component={ SettingsProxy } />
      </Settings>
    </Router>
  </ContextProvider>,
  document.querySelector('#container')
);
