import { Update, Message, CallbackQuery } from '@grammyjs/types';

export type MessageHandler = (msg: Message, state: FSMContext) => Promise<void>;
export type CallbackQueryHandler = (cb: CallbackQuery, state: FSMContext) => Promise<void>;

interface Route {
  command?: string;
  callbackDataPrefix?: string;
  state?: string;
  handler: MessageHandler | CallbackQueryHandler;
}

export class FSMContext {
  private static states = new Map<number, string>();
  private static data = new Map<number, Record<string, any>>();

  constructor(public readonly userId: number) {}

  setState(state: string | null) {
    if (state) {
      FSMContext.states.set(this.userId, state);
    } else {
      FSMContext.states.delete(this.userId);
    }
  }

  getState(): string | null {
    return FSMContext.states.get(this.userId) || null;
  }

  updateData(data: Record<string, any>) {
    const existing = FSMContext.data.get(this.userId) || {};
    FSMContext.data.set(this.userId, { ...existing, ...data });
  }

  getData(): Record<string, any> {
    return FSMContext.data.get(this.userId) || {};
  }

  clear() {
    this.setState(null);
    FSMContext.data.delete(this.userId);
  }
}

export class Router {
  private middlewares: ((u: Update, next: () => Promise<void>) => Promise<void>)[] = [];
  private messageRoutes: Route[] = [];
  private callbackRoutes: Route[] = [];
  private stateRoutes: Route[] = [];

  private genericRoutes: { filter: (u: Update) => boolean, handler: (u: Update) => Promise<void> }[] = [];

  use(middleware: (u: Update, next: () => Promise<void>) => Promise<void>) {
    this.middlewares.push(middleware);
  }

  onCommand(command: string, handler: MessageHandler) {
    this.messageRoutes.push({ command, handler });
  }

  onCallback(dataPrefix: string, handler: CallbackQueryHandler) {
    this.callbackRoutes.push({ callbackDataPrefix: dataPrefix, handler });
  }

  onState(state: string, handler: MessageHandler) {
    this.stateRoutes.push({ state, handler });
  }

  on(filter: (u: Update) => boolean, handler: (u: Update) => Promise<void>) {
    this.genericRoutes.push({ filter, handler });
  }

  async handleUpdate(update: Update) {
    const runRoutes = async () => {
      // Run generic routes
      for (const route of this.genericRoutes) {
        if (route.filter(update)) {
          await route.handler(update);
        }
      }

      if (update.message) {
        const msg = update.message;
        const userId = msg.from?.id;
        if (userId) {
          const fsm = new FSMContext(userId);
          const currentState = fsm.getState();

          if (currentState) {
            const route = this.stateRoutes.find(r => r.state === currentState);
            if (route) {
              await (route.handler as MessageHandler)(msg, fsm);
              return;
            }
          }
        }

        if (msg.text?.startsWith('/')) {
          const cmd = msg.text.split(' ')[0].substring(1);
          const route = this.messageRoutes.find(r => r.command === cmd);
          if (route) {
            const fsm = new FSMContext(msg.from!.id);
            await (route.handler as MessageHandler)(msg, fsm);
            return;
          }
        }
      }

      if (update.callback_query) {
        const cb = update.callback_query;
        const userId = cb.from.id;
        const fsm = new FSMContext(userId);

        const data = cb.data;
        if (data) {
          const route = this.callbackRoutes.find(r => data.startsWith(r.callbackDataPrefix!));
          if (route) {
            await (route.handler as CallbackQueryHandler)(cb, fsm);
          }
        }
      }
    };

    const dispatch = async (index: number): Promise<void> => {
      if (index < this.middlewares.length) {
        await this.middlewares[index](update, () => dispatch(index + 1));
      } else {
        await runRoutes();
      }
    };

    await dispatch(0);
  }
}

export const router = new Router();
