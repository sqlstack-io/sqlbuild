// `?calm`: the mock with nothing needing attention, for the Overview with nothing open or acknowledged.
import type { ApiClient } from "../client";

/** `client`, whose Overview lists nothing needing attention and nothing acknowledged. */
export function calm(client: ApiClient): ApiClient {
  return new Proxy(client, {
    get(target, prop, receiver) {
      const value = Reflect.get(target, prop, receiver);
      if (prop !== "overview" || typeof value !== "function")
        return typeof value === "function" ? value.bind(target) : value;
      return async (...args: Parameters<ApiClient["overview"]>) => {
        const overview = await value.apply(target, args);
        return (
          overview && {
            ...overview,
            attention: [],
            clearedAcknowledgements: [],
          }
        );
      };
    },
  });
}
