// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {WsPing, WsPing_UEBACodec} from './WsPing'
import {Command, Command_UEBACodec} from './Command'

export type WsClientMessage = Ping | SendCommand

export const WsClientMessage = {
    BaboonDomainVersion: '0.2.0',
    BaboonDomainIdentifier: 'victron_controller.dashboard',
    BaboonTypeIdentifier: 'victron_controller.dashboard/:#WsClientMessage',
    BaboonSameInVersions: ["0.2.0", "0.3.0"],
    BaboonAdtTypeIdentifier: 'victron_controller.dashboard/:#WsClientMessage',
    binCodec(): WsClientMessage_UEBACodec {
        return WsClientMessage_UEBACodec.instance
    }
} as const

export function isPing(value: WsClientMessage): value is Ping { return value instanceof Ping; }
export function isSendCommand(value: WsClientMessage): value is SendCommand { return value instanceof SendCommand; }

export class Ping implements BaboonGenerated {
    private readonly _body: WsPing;

    constructor(body: WsPing) {
        this._body = body
    }

    public get body(): WsPing {
        return this._body;
    }

    public toJSON(): Record<string, unknown> {
        return {
            body: this._body
        };
    }

    public with(overrides: {body?: WsPing}): Ping {
        return new Ping(
            'body' in overrides ? overrides.body! : this._body
        );
    }

    public static fromPlain(obj: {body: WsPing}): Ping {
        return new Ping(
            obj.body
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Ping.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Ping.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#Ping'
    public baboonTypeIdentifier() {
        return Ping.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Ping.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#Ping'
    public baboonAdtTypeIdentifier() {
        return Ping.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Ping_UEBACodec {
        return Ping_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Ping_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Ping, writer: BaboonBinWriter): unknown {
        if (this !== Ping_UEBACodec.lazyInstance.value) {
          return Ping_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                WsPing_UEBACodec.instance.encode(ctx, value.body, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            WsPing_UEBACodec.instance.encode(ctx, value.body, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Ping {
        if (this !== Ping_UEBACodec .lazyInstance.value) {
            return Ping_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const body = WsPing_UEBACodec.instance.decode(ctx, reader);
        return new Ping(
            body,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Ping_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Ping_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#Ping'
    public baboonTypeIdentifier() {
        return Ping_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#Ping'
    public baboonAdtTypeIdentifier() {
        return Ping_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Ping_UEBACodec())
    public static get instance(): Ping_UEBACodec {
        return Ping_UEBACodec.lazyInstance.value
    }
}

export class SendCommand implements BaboonGenerated {
    private readonly _body: Command;

    constructor(body: Command) {
        this._body = body
    }

    public get body(): Command {
        return this._body;
    }

    public toJSON(): Record<string, unknown> {
        return {
            body: this._body
        };
    }

    public with(overrides: {body?: Command}): SendCommand {
        return new SendCommand(
            'body' in overrides ? overrides.body! : this._body
        );
    }

    public static fromPlain(obj: {body: Command}): SendCommand {
        return new SendCommand(
            obj.body
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return SendCommand.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SendCommand.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#SendCommand'
    public baboonTypeIdentifier() {
        return SendCommand.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return SendCommand.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#SendCommand'
    public baboonAdtTypeIdentifier() {
        return SendCommand.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): SendCommand_UEBACodec {
        return SendCommand_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class SendCommand_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SendCommand, writer: BaboonBinWriter): unknown {
        if (this !== SendCommand_UEBACodec.lazyInstance.value) {
          return SendCommand_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Command_UEBACodec.instance.encode(ctx, value.body, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            Command_UEBACodec.instance.encode(ctx, value.body, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SendCommand {
        if (this !== SendCommand_UEBACodec .lazyInstance.value) {
            return SendCommand_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const body = Command_UEBACodec.instance.decode(ctx, reader);
        return new SendCommand(
            body,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return SendCommand_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SendCommand_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#SendCommand'
    public baboonTypeIdentifier() {
        return SendCommand_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#WsClientMessage]#SendCommand'
    public baboonAdtTypeIdentifier() {
        return SendCommand_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SendCommand_UEBACodec())
    public static get instance(): SendCommand_UEBACodec {
        return SendCommand_UEBACodec.lazyInstance.value
    }
}


/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class WsClientMessage_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WsClientMessage, writer: BaboonBinWriter): unknown {
        if (this !== WsClientMessage_UEBACodec.lazyInstance.value) {
          return WsClientMessage_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (value instanceof Ping) {
                BinTools.writeByte(writer, 0);
                Ping_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof SendCommand) {
                BinTools.writeByte(writer, 1);
                SendCommand_UEBACodec.instance.encode(ctx, value, writer);
            }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WsClientMessage {
        if (this !== WsClientMessage_UEBACodec .lazyInstance.value) {
            return WsClientMessage_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return Ping_UEBACodec.instance.decode(ctx, reader)
                case 1: return SendCommand_UEBACodec.instance.decode(ctx, reader)
            default: throw new Error("Unknown ADT branch tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return WsClientMessage_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WsClientMessage_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WsClientMessage'
    public baboonTypeIdentifier() {
        return WsClientMessage_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/:#WsClientMessage'
    public baboonAdtTypeIdentifier() {
        return WsClientMessage_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WsClientMessage_UEBACodec())
    public static get instance(): WsClientMessage_UEBACodec {
        return WsClientMessage_UEBACodec.lazyInstance.value
    }
}