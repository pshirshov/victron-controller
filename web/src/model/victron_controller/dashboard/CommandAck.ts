// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class CommandAck implements BaboonGeneratedLatest {
    private readonly _accepted: boolean;
    private readonly _error_message: string | undefined;

    constructor(accepted: boolean, error_message: string | undefined) {
        this._accepted = accepted
        this._error_message = error_message
    }

    public get accepted(): boolean {
        return this._accepted;
    }
    public get error_message(): string | undefined {
        return this._error_message;
    }

    public toJSON(): Record<string, unknown> {
        return {
            accepted: this._accepted,
            error_message: this._error_message !== undefined ? this._error_message : undefined
        };
    }

    public with(overrides: {accepted?: boolean; error_message?: string | undefined}): CommandAck {
        return new CommandAck(
            'accepted' in overrides ? overrides.accepted! : this._accepted,
            'error_message' in overrides ? overrides.error_message! : this._error_message
        );
    }

    public static fromPlain(obj: {accepted: boolean; error_message: string | undefined}): CommandAck {
        return new CommandAck(
            obj.accepted,
            obj.error_message
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CommandAck.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CommandAck.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CommandAck'
    public baboonTypeIdentifier() {
        return CommandAck.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return CommandAck.BaboonSameInVersions
    }
    public static binCodec(): CommandAck_UEBACodec {
        return CommandAck_UEBACodec.instance
    }
}

export class CommandAck_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: CommandAck, writer: BaboonBinWriter): unknown {
        if (this !== CommandAck_UEBACodec.lazyInstance.value) {
          return CommandAck_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeBool(buffer, value.accepted);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.error_message === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.error_message);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeBool(writer, value.accepted);
            if (value.error_message === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.error_message);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): CommandAck {
        if (this !== CommandAck_UEBACodec .lazyInstance.value) {
            return CommandAck_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const accepted = BinTools.readBool(reader);
        const error_message = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        return new CommandAck(
            accepted,
            error_message,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CommandAck_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CommandAck_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CommandAck'
    public baboonTypeIdentifier() {
        return CommandAck_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new CommandAck_UEBACodec())
    public static get instance(): CommandAck_UEBACodec {
        return CommandAck_UEBACodec.lazyInstance.value
    }
}