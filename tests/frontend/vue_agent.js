window.__vue_agent__ = (function () {
  function getApp() {
    var el = document.getElementById('app');
    if (!el || !el.__vue_app__) return null;
    return el.__vue_app__;
  }

  function getRootInstance() {
    var app = getApp();
    if (!app) return null;
    return app._instance;
  }

  function forEachVNode(vnode, fn) {
    if (!vnode) return;
    if (Array.isArray(vnode)) {
      for (var i = 0; i < vnode.length; i++) forEachVNode(vnode[i], fn);
      return;
    }
    if (vnode.component) { fn(vnode.component); return; }
    var children = vnode.children || vnode.dynamicChildren;
    if (children) {
      if (Array.isArray(children)) {
        for (var i = 0; i < children.length; i++) {
          var child = children[i];
          if (child && typeof child === 'object') {
            if (child.component) {
              fn(child.component);
            } else {
              forEachVNode(child, fn);
            }
          }
        }
      } else if (typeof children === 'object' && !Array.isArray(children)) {
        var keys = Object.keys(children);
        for (var i = 0; i < keys.length; i++) forEachVNode(children[keys[i]], fn);
      }
    }
  }

  function walkComponents(instance, callback, depth) {
    if (depth === void 0) depth = 0;
    if (!instance) return;
    callback(instance, depth);
    var processed = new Set();
    forEachVNode(instance.subTree, function (comp) {
      if (!processed.has(comp.uid)) {
        processed.add(comp.uid);
        walkComponents(comp, callback, depth + 1);
      }
    });
  }

  function extractValue(val, depth) {
    if (depth === void 0) depth = 0;
    if (depth > 5) return '[Deep]';
    if (val === null || val === undefined) return val;
    if (typeof val === 'function') return '[Function]';
    if (typeof val === 'symbol') return val.toString();
    if (typeof val !== 'object') return val;
    if (val instanceof HTMLElement) return '[HTMLElement: ' + val.tagName + ']';
    if (val instanceof Window) return '[Window]';
    if (val instanceof Blob) return '[Blob]';
    if (val instanceof Map) return '[Map(' + val.size + ')]';
    if (val instanceof Set) return '[Set(' + val.size + ')]';
    if (val instanceof Error) return '[Error: ' + val.message + ']';
    if (val instanceof Date) return val.toISOString();
    if (Array.isArray(val)) {
      return val.slice(0, 50).map(function (v) { return extractValue(v, depth + 1); });
    }
    if (val.__v_raw !== undefined) {
      try { return extractValue(val.__v_raw, depth); } catch (e) { return '[Reactive]'; }
    }
    if (val.__v_isRef) {
      try { return extractValue(val.value, depth + 1); } catch (e) { return '[Ref]'; }
    }
    try {
      var str = JSON.stringify(val);
      if (str && str.length > 500) return '[Object(' + str.length + ' chars)]';
      return JSON.parse(str);
    } catch (e) {
      var result = {};
      try {
        var keys = Object.keys(val);
        for (var i = 0; i < keys.length; i++) {
          try {
            result[keys[i]] = extractValue(val[keys[i]], depth + 1);
          } catch (e2) {
            result[keys[i]] = '[Error]';
          }
        }
      } catch (e2) {}
      return Object.keys(result).length > 0 ? result : '[Circular]';
    }
  }

  function extractSetupState(instance) {
    var result = {};
    var setupState = instance.setupState;
    if (!setupState) return result;
    var keys = Object.keys(setupState);
    for (var i = 0; i < keys.length; i++) {
      var key = keys[i];
      try {
        var val = setupState[key];
        if (typeof val === 'function' && !val.effect) continue;
        if (val && val.__v_isRef) {
          result[key] = { __type: 'ref', value: extractValue(val.value, 1) };
        } else if (val && val.__v_isReactive) {
          result[key] = { __type: 'reactive', value: extractValue(val, 1) };
        } else {
          result[key] = extractValue(val, 1);
        }
      } catch (e) {
        result[key] = '[Error: ' + e.message + ']';
      }
    }
    return result;
  }

  function extractProps(instance) {
    var result = {};
    var props = instance.props;
    if (!props) return result;
    var keys = Object.keys(props);
    for (var i = 0; i < keys.length; i++) {
      try {
        result[keys[i]] = extractValue(props[keys[i]]);
      } catch (e) {
        result[keys[i]] = '[Error]';
      }
    }
    return result;
  }

  function serializeComponent(instance, depth) {
    var name = instance.type ? (instance.type.name || instance.type.__name || instance.type.__file || 'Anonymous') : 'Unknown';
    if (name.includes('\\') || name.includes('/')) {
      name = name.split(/[/\\]/).pop().replace('.vue', '');
    }
    return {
      id: instance.uid,
      name: name,
      depth: depth,
      props: extractProps(instance),
      setupState: extractSetupState(instance),
    };
  }

  function findComponentsByName(name) {
    var root = getRootInstance();
    if (!root) return [];
    var results = [];
    walkComponents(root, function (instance, depth) {
      var compName = instance.type ? (instance.type.name || instance.type.__name || '') : '';
      if (!compName && instance.type && instance.type.__file) {
        compName = instance.type.__file.split(/[/\\]/).pop().replace('.vue', '');
      }
      if (compName === name) {
        results.push(serializeComponent(instance, depth));
      }
    });
    return results;
  }

  function findComponentByRoute(routeName) {
    var root = getRootInstance();
    if (!root) return null;
    var found = null;
    walkComponents(root, function (instance) {
      if (found) return;
      var matchesRoute = false;
      if (instance.setupState && instance.setupState.router) {
        var currentRoute = instance.setupState.router.currentRoute;
        if (currentRoute && currentRoute.value && currentRoute.value.name === routeName) {
          matchesRoute = true;
        }
      }
      if (instance.setupState && instance.setupState.route) {
        var route = instance.setupState.route;
        if (route && route.value && route.value.name === routeName) {
          matchesRoute = true;
        }
      }
      if (matchesRoute) {
        found = serializeComponent(instance, 0);
      }
    });
    return found;
  }

  function getComponentState(uid) {
    var root = getRootInstance();
    if (!root) return null;
    var result = null;
    walkComponents(root, function (instance) {
      if (instance.uid === uid) {
        result = serializeComponent(instance, 0);
      }
    });
    return result;
  }

  function callMethod(uid, methodName, args) {
    var root = getRootInstance();
    if (!root) return { error: 'No Vue app found' };
    var found = null;
    walkComponents(root, function (instance) {
      if (instance.uid === uid) found = instance;
    });
    if (!found) return { error: 'Component not found: uid=' + uid };
    var method = found.setupState ? found.setupState[methodName] : null;
    if (!method || typeof method !== 'function') return { error: 'Method not found: ' + methodName };
    try {
      var result = method.apply(null, args || []);
      if (result && typeof result.then === 'function') {
        return result.then(function (r) { return { success: true, result: extractValue(r) }; });
      }
      return { success: true, result: extractValue(result) };
    } catch (e) {
      return { error: e.message };
    }
  }

  function setRef(uid, key, value) {
    var root = getRootInstance();
    if (!root) return { error: 'No Vue app found' };
    var found = null;
    walkComponents(root, function (instance) {
      if (instance.uid === uid) found = instance;
    });
    if (!found) return { error: 'Component not found: uid=' + uid };
    var setupState = found.setupState;
    if (!setupState) return { error: 'No setupState' };
    var val = setupState[key];
    if (val === undefined) return { error: 'Key not found: ' + key };
    if (val !== null && typeof val === 'object' && val.__v_isRef) {
      val.value = value;
      return { success: true };
    }
    var raw = setupState.__v_raw || setupState;
    var rawVal = raw[key];
    if (rawVal !== null && typeof rawVal === 'object' && rawVal.__v_isRef) {
      rawVal.value = value;
      return { success: true };
    }
    raw[key] = value;
    return { success: true };
  }

  function setReactiveField(uid, key, field, value) {
    var root = getRootInstance();
    if (!root) return { error: 'No Vue app found' };
    var found = null;
    walkComponents(root, function (instance) {
      if (instance.uid === uid) found = instance;
    });
    if (!found) return { error: 'Component not found: uid=' + uid };
    var obj = found.setupState ? found.setupState[key] : null;
    if (!obj) return { error: 'Reactive not found: ' + key };
    obj[field] = value;
    return { success: true };
  }

  function getStore(storeName) {
    var app = getApp();
    if (!app) return null;
    var pinia = app.config.globalProperties.$pinia;
    if (!pinia) return null;
    var store = pinia._s.get(storeName);
    if (!store) return null;
    var result = {};
    var keys = Object.keys(store);
    for (var i = 0; i < keys.length; i++) {
      var key = keys[i];
      if (key.startsWith('$') || key.startsWith('_')) continue;
      try {
        var val = store[key];
        if (typeof val === 'function') continue;
        result[key] = extractValue(val, 1);
      } catch (e) {
        result[key] = '[Error]';
      }
    }
    return result;
  }

  function dispatchStore(storeName, actionName, args) {
    var app = getApp();
    if (!app) return { error: 'No Vue app found' };
    var pinia = app.config.globalProperties.$pinia;
    if (!pinia) return { error: 'No Pinia found' };
    var store = pinia._s.get(storeName);
    if (!store) return { error: 'Store not found: ' + storeName };
    var action = store[actionName];
    if (!action || typeof action !== 'function') return { error: 'Action not found: ' + actionName };
    try {
      var result = action.apply(store, args || []);
      if (result && typeof result.then === 'function') {
        return result.then(function (r) { return { success: true, result: extractValue(r) }; });
      }
      return { success: true, result: extractValue(result) };
    } catch (e) {
      return { error: e.message };
    }
  }

  function listAllComponents() {
    var root = getRootInstance();
    if (!root) return [];
    var results = [];
    walkComponents(root, function (instance, depth) {
      results.push(serializeComponent(instance, depth));
    });
    return results;
  }

  function listStores() {
    var app = getApp();
    if (!app) return [];
    var pinia = app.config.globalProperties.$pinia;
    if (!pinia) return [];
    var names = [];
    pinia._s.forEach(function (_, key) { names.push(key); });
    return names;
  }

  function isReady() {
    return getApp() !== null;
  }

  return {
    isReady: isReady,
    listAllComponents: listAllComponents,
    findComponentsByName: findComponentsByName,
    findComponentByRoute: findComponentByRoute,
    getComponentState: getComponentState,
    callMethod: callMethod,
    setRef: setRef,
    setReactiveField: setReactiveField,
    getStore: getStore,
    listStores: listStores,
    dispatchStore: dispatchStore,
  };
})();
