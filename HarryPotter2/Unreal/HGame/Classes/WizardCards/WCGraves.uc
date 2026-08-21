//===============================================================================
//  #94 Merton Graves
//===============================================================================

class  WCGraves extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardGravesTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Gravessmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardGravesBigTexture FILE=TEXTURES\menu\Folio\Cards\Gravesbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Merton Graves";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=94
    skin(0)=Texture'HProps.skins.WizardCardGravesTex0'
	textureBig=Texture'WizCardGravesBigTexture'
	strDescriptionId="WizCard_0015"
}
